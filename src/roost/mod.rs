pub mod store;

use starling::config::Profile;
use starling::crypto::FlockCrypto;
use starling::event::GossipPayload;
use starling::net::{room_code_from_node_id, topic_for};
use iroh::{Endpoint, endpoint::presets, protocol::Router};
use iroh_gossip::api::Event;
use iroh_gossip::net::{GOSSIP_ALPN, Gossip};
use n0_future::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use store::Store;

#[derive(Clone, Serialize, Deserialize)]
pub struct RoostState {
    pub name: String,
    pub channels: Vec<String>,
}

fn roost_data_dir(name: &str) -> PathBuf {
    Profile::roosts_dir().join(name)
}

fn roost_db_path(name: &str) -> PathBuf {
    roost_data_dir(name).join("roost.db")
}

fn roost_key_path(name: &str) -> PathBuf {
    roost_data_dir(name).join("identity.key")
}

fn load_invite_code(name: &str) -> anyhow::Result<String> {
    let key_path = roost_key_path(name);
    let bytes = std::fs::read(&key_path).map_err(|e| {
        anyhow::anyhow!(
            "roost '{name}' has no identity key at {}: {e}",
            key_path.display()
        )
    })?;
    let arr: [u8; 32] = bytes[..32]
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid identity key file (expected 32 bytes)"))?;
    let secret = iroh::SecretKey::from_bytes(&arr);
    let node_id: iroh::EndpointId = secret.public().into();
    Ok(room_code_from_node_id(&node_id))
}

pub fn create(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if dir.exists() {
        anyhow::bail!("roost '{name}' already exists at {}", dir.display());
    }

    std::fs::create_dir_all(&dir)?;
    let _db = sled::open(&roost_db_path(name))?;
    starling::logger::info(&format!(
        "created roost database at {}",
        roost_db_path(name).display()
    ));

    let key = iroh::SecretKey::generate();
    std::fs::write(&roost_key_path(name), key.to_bytes())?;

    let node_id: iroh::EndpointId = key.public().into();
    let code = room_code_from_node_id(&node_id);
    println!("✓ roost '{name}' created");
    println!("  invite code: {code}");
    println!("  data: {}", dir.display());
    println!();
    println!("Start it with: starling-server roost open {name}");
    starling::logger::info(&format!("roost '{name}' created with code {code}"));

    Ok(())
}

pub async fn open(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!(
            "roost '{name}' not found at {}. Create it first with: starling-server roost create {name}",
            dir.display()
        );
    }

    starling::logger::info(&format!("opening roost '{name}' from {}", dir.display()));

    let store = Arc::new(Store::open(&roost_db_path(name).to_string_lossy())?);

    let state = RoostState {
        name: name.to_string(),
        channels: vec!["general".into()],
    };

    let secret = match std::fs::read(&roost_key_path(name)) {
        Ok(bytes) if bytes.len() == 32 => {
            let arr: [u8; 32] = bytes.try_into().expect("checked len");
            iroh::SecretKey::from_bytes(&arr)
        }
        _ => {
            let key = iroh::SecretKey::generate();
            if let Err(e) = std::fs::write(&roost_key_path(name), key.to_bytes()) {
                starling::logger::error(&format!("failed to write roost identity key: {e}"));
            }
            key
        }
    };

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret)
        .bind()
        .await
        .map_err(|e| {
            starling::logger::error(&format!("endpoint bind failed for roost '{name}': {e}"));
            e
        })?;
    endpoint.online().await;

    let my_id = endpoint.addr().id;
    let code = room_code_from_node_id(&my_id);
    println!("✓ roost '{name}' is online");
    println!("  code: {code}");
    println!("  join: starling join {code}");
    starling::logger::info(&format!("roost '{name}' online with code {code}"));

    let gossip = Gossip::builder().spawn(endpoint.clone());
    let _router = Router::builder(endpoint.clone())
        .accept(GOSSIP_ALPN, gossip.clone())
        .accept(
            RoostSync::ALPN,
            RoostSync {
                store: store.clone(),
            },
        )
        .spawn();

    for chan in &state.channels {
        let topic = topic_for(&format!("starling/roost/{code}/{chan}"));
        let crypto = FlockCrypto::from_room_code(&format!("{code}/{chan}"));
        let (_sender, mut rx) = gossip.subscribe(topic, vec![]).await?.split();
        let (st, ch) = (store.clone(), chan.clone());

        tokio::spawn(async move {
            while let Some(Ok(Event::Received(msg))) = rx.next().await {
                if let Some(plain) = crypto.decrypt(&msg.content) {
                    match postcard::from_bytes::<GossipPayload>(&plain) {
                        Ok(GossipPayload::Chat(m)) => {
                            if let Err(e) = st.append(&ch, &m) {
                                starling::logger::error(&format!(
                                    "roost: failed to persist message in '{ch}': {e}"
                                ));
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            starling::logger::warn(&format!(
                                "roost: failed to deserialize gossip payload: {e}"
                            ));
                        }
                    }
                }
            }
            starling::logger::warn(&format!(
                "roost: gossip subscription ended for channel '{ch}'"
            ));
        });
    }

    let control = topic_for(&format!("starling/roost/{code}/_control"));
    let ctl_crypto = FlockCrypto::from_room_code(&format!("{code}/_control"));
    let (ctl_tx, mut ctl_rx) = gossip.subscribe(control, vec![]).await?.split();

    loop {
        tokio::select! {
            Some(Ok(Event::NeighborUp(_))) = ctl_rx.next() => {
                match postcard::to_stdvec(&state) {
                    Ok(blob) => {
                        let encrypted = ctl_crypto.encrypt(&blob);
                        if let Err(e) = ctl_tx.broadcast(encrypted.into()).await {
                            starling::logger::warn(&format!(
                                "roost: failed to broadcast state on control channel: {e}"
                            ));
                        }
                    }
                    Err(e) => {
                        starling::logger::error(&format!(
                            "roost: failed to serialise roost state: {e}"
                        ));
                    }
                }
            }
            else => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

pub fn destroy(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }
    std::fs::remove_dir_all(&dir)?;
    println!("✓ roost '{name}' destroyed");
    starling::logger::warn(&format!("roost '{name}' destroyed by user"));
    Ok(())
}

pub fn invite(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }
    let code = load_invite_code(name)?;
    println!("roost '{name}' invite code:");
    println!("  {code}");
    println!();
    println!("Join with: starling join {code}");
    Ok(())
}

pub fn status(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }

    let code = load_invite_code(name).unwrap_or_else(|_| "(unknown)".into());
    let db_size = roost_db_path(name).metadata().map(|m| m.len()).unwrap_or(0);

    println!("roost '{name}'");
    println!("  path:   {}", dir.display());
    println!("  code:   {code}");
    println!("  db:     {} bytes", db_size);
    Ok(())
}

pub fn doctor(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }

    let mut issues = Vec::new();

    let key_path = roost_key_path(name);
    if key_path.exists() {
        let meta = key_path
            .metadata()
            .map_err(|e| anyhow::anyhow!("can't read identity key metadata: {e}"))?;
        if meta.len() != 32 {
            issues.push(format!(
                "identity key has wrong size ({} bytes, expected 32)",
                meta.len()
            ));
        }
    } else {
        issues.push("identity key missing".into());
    }

    let db_path = roost_db_path(name);
    if db_path.exists() {
        match sled::open(&db_path) {
            Ok(db) => {
                let count = db.iter().count();
                println!("  database: ✓ ({} entries)", count);
                drop(db);
            }
            Err(e) => {
                issues.push(format!("database corrupt or unreadable: {e}"));
            }
        }
    } else {
        issues.push("database file missing".into());
    }

    if issues.is_empty() {
        println!("✓ roost '{name}' looks healthy");
    } else {
        println!("✗ roost '{name}' has issues:");
        for issue in &issues {
            println!("    - {issue}");
        }
    }
    Ok(())
}

pub fn logs(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }
    println!("roost '{name}' logs:");
    println!("  Logs are written to logs/latest.log in the working directory");
    Ok(())
}

#[derive(Debug, Clone)]
struct RoostSync {
    store: Arc<Store>,
}

impl RoostSync {
    const ALPN: &[u8] = b"starling/roost-sync/0";
}

impl iroh::protocol::ProtocolHandler for RoostSync {
    async fn accept(
        &self,
        conn: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        let Ok((mut send, mut recv)) = conn.accept_bi().await else {
            starling::logger::warn("roost-sync: failed to accept bi stream");
            return Ok(());
        };

        let Ok(req) = recv.read_to_end(256).await else {
            starling::logger::warn("roost-sync: failed to read request");
            return Ok(());
        };
        let Ok((chan, since)): Result<(String, i64), _> = postcard::from_bytes(&req) else {
            starling::logger::warn("roost-sync: invalid request format");
            return Ok(());
        };

        let history = self.store.since(&chan, since);
        match postcard::to_stdvec(&history) {
            Ok(bytes) => {
                if let Err(e) = send.write_all(&bytes).await {
                    starling::logger::warn(&format!("roost-sync: failed to send history: {e}"));
                }
                let _ = send.finish();
            }
            Err(e) => {
                starling::logger::error(&format!("roost-sync: failed to serialise history: {e}"));
            }
        }

        conn.closed().await;
        Ok(())
    }
}
