//! Roost mode — a persistent headless bird that stays online, stores
//! message history to disk, and serves it to late-joining peers.
//!
//! A roost is Starling's answer to a Discord server: a named, always-on
//! node that holds a community's chat history. Unlike a regular flock
//! (which is ephemeral — here while birds are online), a roost persists
//! everything to a sled database so history survives restarts.
//!
//! ## CLI usage (from `starling-server`)
//!
//! ```text
//! starling-server roost create  <name>   — create a new roost
//! starling-server roost open    <name>   — start a roost server (blocks)
//! starling-server roost close   <name>   — stop a running roost
//! starling-server roost destroy <name>   — delete a roost and all data
//! starling-server roost invite  <name>   — show the roost's invite code
//! starling-server roost doctor  [name]   — diagnose roost(s)
//! starling-server roost setup   <name>   — interactive create / configure
//! ```
//!
//! Each roost lives in its own data directory under the Starling config
//! folder (`~/.config/starling/roosts/<name>/` on Unix,
//! `%APPDATA%/starling/roosts/<name>/` on Windows) and gets its own
//! cryptographic identity key — separate from the user's personal key.

pub mod store;

use crate::config::Profile;
use crate::crypto::FlockCrypto;
use crate::event::GossipPayload;
use crate::logger;
use crate::net::{room_code_from_node_id, topic_for};
use iroh::{Endpoint, endpoint::presets, protocol::Router};
use iroh_gossip::api::Event;
use iroh_gossip::net::{GOSSIP_ALPN, Gossip};
use n0_future::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use store::Store;

/// Metadata about a roost that gets broadcast on the control channel so
/// clients can render the flock rail (the channel list on the left).
///
/// In later phases, this struct will be signed so clients can verify
/// it came from the roost's identity key.
#[derive(Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RoostState {
    /// Human-friendly display name for this roost.
    pub name: String,
    /// Ordered list of channel names (e.g. `["general", "builds", ...]`).
    pub channels: Vec<String>,
}

/// ── Helpers ────────────────────────────────────────────────────────────

/// Return the data directory for a named roost.
///
/// Layout: `<config>/roosts/<name>/` where `<config>` is the standard
/// Starling config directory (`~/.config/starling` or `%APPDATA%/starling`).
fn roost_data_dir(name: &str) -> PathBuf {
    Profile::roosts_dir().join(name)
}

/// Path to the sled database inside a roost's data directory.
fn roost_db_path(name: &str) -> PathBuf {
    roost_data_dir(name).join("roost.db")
}

/// Path to the Ed25519 identity key for a roost.
fn roost_key_path(name: &str) -> PathBuf {
    roost_data_dir(name).join("identity.key")
}

/// Load the identity key for a named roost and return its invite code.
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

/// ── Public API ─────────────────────────────────────────────────────────

/// Create a new roost on disk.
///
/// This initialises the data directory, generates a dedicated
/// cryptographic identity (so the roost's invite code stays the same
/// across restarts), and creates the sled database. The roost is then
/// ready to be opened with [`open`].
///
/// # Errors
///
/// Returns an error if a roost with the same name already exists, or if
/// filesystem operations fail.
pub fn create(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if dir.exists() {
        anyhow::bail!("roost '{name}' already exists at {}", dir.display());
    }

    // Create the data directory and initialise the database.
    std::fs::create_dir_all(&dir)?;
    let _db = sled::open(&roost_db_path(name))?;
    logger::info(&format!(
        "created roost database at {}",
        roost_db_path(name).display()
    ));

    // Generate a dedicated identity key for this roost so its invite
    // code is stable across restarts.
    let key = iroh::SecretKey::generate();
    std::fs::write(&roost_key_path(name), key.to_bytes())?;

    let node_id: iroh::EndpointId = key.public().into();
    let code = room_code_from_node_id(&node_id);
    println!("✓ roost '{name}' created");
    println!("  invite code: {code}");
    println!("  data: {}", dir.display());
    println!();
    println!("Start it with: starling-server roost open {name}");
    logger::info(&format!("roost '{name}' created with code {code}"));

    Ok(())
}

/// Start a headless roost server.
///
/// Loads the roost's data directory by name, binds an iroh endpoint
/// using the roost's dedicated identity key, subscribes to gossip
/// topics for all channels, and persists every incoming message to
/// the sled database.
///
/// The server runs until the process receives Ctrl+C (SIGINT).
///
/// # Panics
///
/// Panics if the roost data directory doesn't exist — run [`create`]
/// first.
pub async fn open(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!(
            "roost '{name}' not found at {}. Create it first with: starling-server roost create {name}",
            dir.display()
        );
    }

    logger::info(&format!("opening roost '{name}' from {}", dir.display()));

    // Open the sled-backed message store.
    let store = Arc::new(Store::open(&roost_db_path(name).to_string_lossy())?);

    let state = RoostState {
        name: name.to_string(),
        channels: vec!["general".into()],
    };

    // Load the roost's dedicated identity key, or generate one if this
    // is the very first open after creation (paranoid fallback).
    let secret = match std::fs::read(&roost_key_path(name)) {
        Ok(bytes) if bytes.len() == 32 => {
            let arr: [u8; 32] = bytes.try_into().expect("checked len");
            iroh::SecretKey::from_bytes(&arr)
        }
        _ => {
            let key = iroh::SecretKey::generate();
            if let Err(e) = std::fs::write(&roost_key_path(name), key.to_bytes()) {
                logger::error(&format!("failed to write roost identity key: {e}"));
            }
            key
        }
    };

    // Bind the iroh endpoint using the roost's dedicated identity.
    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret)
        .bind()
        .await
        .map_err(|e| {
            logger::error(&format!("endpoint bind failed for roost '{name}': {e}"));
            e
        })?;
    endpoint.online().await;

    let my_id = endpoint.addr().id;
    let code = room_code_from_node_id(&my_id);
    println!("✓ roost '{name}' is online");
    println!("  code: {code}");
    println!("  join: starling join {code}");
    logger::info(&format!("roost '{name}' online with code {code}"));

    // Spawn the gossip subsystem and register the history sync handler.
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

    // Subscribe to each channel's gossip topic and persist every
    // incoming message to the sled database.
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
                                logger::error(&format!(
                                    "roost: failed to persist message in '{ch}': {e}"
                                ));
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            logger::warn(&format!(
                                "roost: failed to deserialize gossip payload: {e}"
                            ));
                        }
                    }
                }
            }
            logger::warn(&format!(
                "roost: gossip subscription ended for channel '{ch}'"
            ));
        });
    }

    // The control channel: broadcast the roost's channel list whenever
    // a new bird subscribes, so their UI can render the flock rail.
    let control = topic_for(&format!("starling/roost/{code}/_control"));
    let ctl_crypto = FlockCrypto::from_room_code(&format!("{code}/_control"));
    let (ctl_tx, mut ctl_rx) = gossip.subscribe(control, vec![]).await?.split();

    // Block until Ctrl+C kills the process.
    loop {
        tokio::select! {
            Some(Ok(Event::NeighborUp(_))) = ctl_rx.next() => {
                match postcard::to_stdvec(&state) {
                    Ok(blob) => {
                        let encrypted = ctl_crypto.encrypt(&blob);
                        if let Err(e) = ctl_tx.broadcast(encrypted.into()).await {
                            logger::warn(&format!(
                                "roost: failed to broadcast state on control channel: {e}"
                            ));
                        }
                    }
                    Err(e) => {
                        logger::error(&format!(
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

/// Permanently delete a roost and all its data.
pub fn destroy(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }
    std::fs::remove_dir_all(&dir)?;
    println!("✓ roost '{name}' destroyed");
    logger::warn(&format!("roost '{name}' destroyed by user"));
    Ok(())
}

/// Print the roost's invite code.
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

/// Show brief status information about a roost.
pub fn status(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }

    let code = load_invite_code(name).unwrap_or_else(|_| "(unknown)".into());
    let db_size = roost_db_path(name).metadata().map(|m| m.len()).unwrap_or(0);
    let channel_count = 1; // TODO: read from persistent config

    println!("roost '{name}'");
    println!("  path:   {}", dir.display());
    println!("  code:   {code}");
    println!("  db:     {} bytes", db_size);
    println!("  channels: {channel_count}");
    Ok(())
}

/// Basic health diagnostics for a roost.
pub fn doctor(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }

    let mut issues = Vec::new();

    // Check identity key.
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

    // Check database.
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

/// Show the log file path for the roost.
pub fn logs(name: &str) -> anyhow::Result<()> {
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }
    println!("roost '{name}' logs:");
    println!("  Logs are written to logs/latest.log in the working directory");
    println!("  or to the roost's data directory in a future release.");
    Ok(())
}

/// ── Sync protocol ──────────────────────────────────────────────────────

/// Disk-backed sync handler.
///
/// Clients connect, send a `(channel, since_timestamp)` pair, and
/// receive every message in that channel after the given timestamp.
/// The roost answers from its sled database.
#[derive(Debug, Clone)]
struct RoostSync {
    store: Arc<Store>,
}

impl RoostSync {
    /// ALPN string clients use to identify this protocol.
    const ALPN: &[u8] = b"starling/roost-sync/0";
}

impl iroh::protocol::ProtocolHandler for RoostSync {
    async fn accept(
        &self,
        conn: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        let Ok((mut send, mut recv)) = conn.accept_bi().await else {
            logger::warn("roost-sync: failed to accept bi stream");
            return Ok(());
        };

        let Ok(req) = recv.read_to_end(256).await else {
            logger::warn("roost-sync: failed to read request");
            return Ok(());
        };
        let Ok((chan, since)): Result<(String, i64), _> = postcard::from_bytes(&req) else {
            logger::warn("roost-sync: invalid request format");
            return Ok(());
        };

        let history = self.store.since(&chan, since);
        match postcard::to_stdvec(&history) {
            Ok(bytes) => {
                if let Err(e) = send.write_all(&bytes).await {
                    logger::warn(&format!("roost-sync: failed to send history: {e}"));
                }
                let _ = send.finish();
            }
            Err(e) => {
                logger::error(&format!("roost-sync: failed to serialise history: {e}"));
            }
        }

        conn.closed().await;
        Ok(())
    }
}
