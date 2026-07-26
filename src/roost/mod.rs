pub mod history_proto;
pub mod history_store;
pub mod store;

use history_proto::HistoryProto;
use history_store::HistoryStore;
use iroh::{Endpoint, RelayMode, RelayUrl, endpoint::presets, protocol::Router};
use iroh_gossip::api::Event;
use iroh_gossip::net::{GOSSIP_ALPN, Gossip};
use n0_future::StreamExt;
use serde::{Deserialize, Serialize};
use starling::config::Profile;
use starling::crypto::FlockCrypto;
use starling::event::GossipPayload;
use starling::history::HISTORY_V1_ALPN;
use starling::net::{encode_roost_code, receive_payload, topic_for};
use starling::roost::perms::Perm;
use starling::roost::{ModRequest, RoostState, RoostWelcome};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use store::Store;

fn validate_roost_name(name: &str) -> anyhow::Result<()> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if !valid {
        anyhow::bail!("invalid roost name: use 1-64 ASCII letters, numbers, '-' or '_'");
    }
    Ok(())
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

fn write_secret_key(path: &std::path::Path, bytes: &[u8; 32]) -> anyhow::Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn load_invite_code(name: &str) -> anyhow::Result<String> {
    validate_roost_name(name)?;
    let key_path = roost_key_path(name);
    let bytes = std::fs::read(&key_path).map_err(|e| {
        anyhow::anyhow!(
            "roost '{name}' has no identity key at {}: {e}",
            key_path.display()
        )
    })?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid identity key file (expected exactly 32 bytes)"))?;
    let secret = iroh::SecretKey::from_bytes(&arr);
    let node_id: iroh::EndpointId = secret.public();
    Ok(encode_roost_code(&node_id))
}

pub fn create(name: &str) -> anyhow::Result<()> {
    validate_roost_name(name)?;
    let dir = roost_data_dir(name);
    if dir.exists() {
        anyhow::bail!("roost '{name}' already exists at {}", dir.display());
    }

    std::fs::create_dir_all(&dir)?;
    let result = create_contents(name, &dir);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    result
}

fn create_contents(name: &str, dir: &std::path::Path) -> anyhow::Result<()> {
    let db = sled::open(roost_db_path(name))?;
    starling::logger::info(&format!(
        "created roost database at {}",
        roost_db_path(name).display()
    ));

    drop(db);
    let key = iroh::SecretKey::generate();
    write_secret_key(&roost_key_path(name), &key.to_bytes())?;

    let node_id: iroh::EndpointId = key.public();
    let code = encode_roost_code(&node_id);
    println!("' roost '{name}' created");
    println!("  invite code: {code}");
    println!("  data: {}", dir.display());
    println!();
    println!("Start it with: starling-server roost open {name}");
    starling::logger::info(&format!(
        "roost '{name}' created, invite fingerprint {}",
        starling::logger::fingerprint(code.as_bytes())
    ));

    Ok(())
}

pub async fn open(name: &str) -> anyhow::Result<()> {
    validate_roost_name(name)?;
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!(
            "roost '{name}' not found at {}. Create it first with: starling-server roost create {name}",
            dir.display()
        );
    }

    starling::logger::info(&format!("opening roost '{name}' from {}", dir.display()));

    let store = Arc::new(Store::open(roost_db_path(name))?);
    let history_store = Arc::new(HistoryStore::new(store.db())?);

    // The owner is the roost's own node id, known only after the endpoint binds.
    // `perms.owner` is filled in below once `my_id` is available. Any persisted
    // roles/members/invitations/bans are loaded so the door survives a restart.
    let persisted_perms = store.load_perms().unwrap_or_else(|e| {
        starling::logger::warn(&format!("roost: failed to load persisted perms: {e}"));
        None
    });
    let state = RoostState {
        name: name.to_string(),
        channels: vec!["general".into()],
        perms: persisted_perms.unwrap_or_default(),
    };
    let state = Arc::new(Mutex::new(state));

    let key_path = roost_key_path(name);
    let bytes = std::fs::read(&key_path).map_err(|e| {
        anyhow::anyhow!(
            "failed to read roost identity at {}: {e}; run `roost doctor` and restore the key from backup",
            key_path.display()
        )
    })?;
    let key_bytes: [u8; 32] = bytes.try_into().map_err(|_| {
        anyhow::anyhow!(
            "invalid roost identity at {} (expected exactly 32 bytes); refusing to change the roost identity",
            key_path.display()
        )
    })?;
    let secret = iroh::SecretKey::from_bytes(&key_bytes);

    let mut builder = Endpoint::builder(presets::N0).secret_key(secret);
    // Allow a community to point its roost's endpoint at a self-hosted
    // iroh-relay (run beside the roost) without rebuilding. Relays only
    // forward ciphertext the E2E crypto has already sealed, so this drops
    // the last centralized dependency in the roost flight path. Mirrors the
    // client override in Starling-TUI/src/net.rs for parity.
    if let Ok(url) = std::env::var("STARLING_RELAY") {
        let relay: RelayUrl = url.parse()?;
        builder = builder.relay_mode(RelayMode::Custom(relay.into()));
    }
    let endpoint = builder.bind().await.map_err(|e| {
        starling::logger::error(&format!("endpoint bind failed for roost '{name}': {e}"));
        e
    })?;
    endpoint.online().await;

    let my_id = endpoint.addr().id;
    {
        let mut st = state.lock().unwrap();
        st.perms.owner = Some(my_id);
    }
    let code = encode_roost_code(&my_id);
    println!("✓ roost '{name}' is online");
    println!("  code: {code}");
    println!("  join: starling join {code}");
    starling::logger::info(&format!(
        "roost '{name}' online, invite fingerprint {}",
        starling::logger::fingerprint(code.as_bytes())
    ));

    let gossip = Gossip::builder().spawn(endpoint.clone());
    // MembershipState V1 is not yet produced by this V0 roost runtime. Keep
    // History V1 registered for interoperability, but deny every request until
    // the authority path injects a membership state and supplies authorization.
    let history = HistoryProto::new(history_store, |_remote, _request, _membership| false);
    let _router = Router::builder(endpoint.clone())
        .accept(GOSSIP_ALPN, gossip.clone())
        .accept(HISTORY_V1_ALPN, history)
        .accept(
            ROOST_SYNC_ALPN,
            RoostSync {
                store: store.clone(),
                state: state.clone(),
            },
        )
        .accept(
            MOD_ALPN,
            ModProto {
                state: state.clone(),
                store: store.clone(),
            },
        )
        .accept(
            JOIN_ALPN,
            JoinProto {
                state: state.clone(),
                store: store.clone(),
            },
        )
        .spawn();

    // The channel list is captured once at startup; live channel add/remove is
    // a follow-on. Secrets are minted and persisted by the store, never derived
    // from the public code, so non-members can't decrypt channel gossip.
    let startup_channels = state.lock().unwrap().channels.clone();
    for chan in &startup_channels {
        let topic = topic_for(&format!("starling/roost/{code}/{chan}"));
        let secret = match store.channel_secret(chan) {
            Ok(secret) => secret,
            Err(e) => {
                starling::logger::error(&format!(
                    "roost: failed to load channel secret for '{chan}': {e}"
                ));
                continue;
            }
        };
        let crypto = FlockCrypto::from_secret(&secret);
        let (_sender, mut rx) = gossip.subscribe(topic, vec![]).await?.split();
        let (st, ch) = (store.clone(), chan.clone());

        tokio::spawn(async move {
            while let Some(Ok(Event::Received(msg))) = rx.next().await {
                // Phase 9: clients broadcast `postcard(Signed)` envelopes. We
                // authenticate the signature before persisting so a forged
                // `ChatMessage.author` attributed to another bird never lands
                // in history. A legacy unsigned `GossipPayload` still decrypts
                // (older peers); we persist those the legacy way to keep a V0
                // roost that broadcasts during migration readable.
                match receive_payload(&crypto, &msg.content) {
                    Ok(Some(envelope)) => match envelope.payload {
                        GossipPayload::Chat(m) => {
                            if let Err(e) = st.append(&ch, &m) {
                                starling::logger::error(&format!(
                                    "roost: failed to persist message in '{ch}': {e}"
                                ));
                            }
                        }
                        _ => {}
                    },
                    Ok(None) => {
                        if let Some(plain) = crypto.decrypt(&msg.content)
                            && let Ok(GossipPayload::Chat(m)) =
                                postcard::from_bytes::<GossipPayload>(&plain)
                        {
                            starling::logger::warn(&format!(
                                "roost: persisting legacy unsigned chat from anonymous peer in '{ch}'"
                            ));
                            if let Err(e) = st.append(&ch, &m) {
                                starling::logger::error(&format!(
                                    "roost: failed to persist message in '{ch}': {e}"
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        starling::logger::warn(&format!(
                            "roost: gossip frame rejected on channel '{ch}': {e}"
                        ));
                    }
                }
            }
            starling::logger::warn(&format!(
                "roost: gossip subscription ended for channel '{ch}'"
            ));
        });
    }

    let control_key = format!("{code}/_control");
    let control = topic_for(&format!("starling/roost/{control_key}"));
    // Phase 9: the control channel (where RoostState — including the ban list
    // and member roster — is broadcast) is now encrypted with a high-entropy
    // secret minted by the store, not derivable from the public roost code.
    // A non-member who merely knows the invite code can no longer read the
    // member list. The secret is handed to admitted birds through the join
    // handshake alongside the per-channel secrets.
    let control_secret = store.control_secret()?;
    let ctl_crypto = FlockCrypto::from_secret(&control_secret);
    let (ctl_tx, mut ctl_rx) = gossip.subscribe(control, vec![]).await?.split();

    loop {
        tokio::select! {
            event = ctl_rx.next() => {
                match event {
                    Some(Ok(Event::NeighborUp(_))) => {
                        let snapshot = state.lock().unwrap().clone();
                        match postcard::to_stdvec(&snapshot) {
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
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        starling::logger::warn(&format!(
                            "roost: control subscription error: {e}"
                        ));
                    }
                    None => {
                        starling::logger::warn("roost: control subscription ended");
                        return Ok(());
                    }
                }
            }
        }
    }
}

pub fn destroy(name: &str) -> anyhow::Result<()> {
    validate_roost_name(name)?;
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
    validate_roost_name(name)?;
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
    validate_roost_name(name)?;
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
    validate_roost_name(name)?;
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
    validate_roost_name(name)?;
    let dir = roost_data_dir(name);
    if !dir.exists() {
        anyhow::bail!("roost '{name}' not found at {}", dir.display());
    }
    println!("roost '{name}' logs:");
    let log_path = starling::config::Profile::config_dir().join("logs/latest.log");
    println!("  {}", log_path.display());
    Ok(())
}

/// ALPN used by clients to request persisted channel history.
pub const ROOST_SYNC_ALPN: &[u8] = b"starling/roost-sync/0";
const MAX_ROOST_SYNC_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Wire request for roost history. Responses are postcard-encoded
/// `Vec<starling::event::ChatMessage>` values capped at 500 messages.
#[derive(Debug, Serialize, Deserialize)]
pub struct RoostSyncRequest {
    pub channel: String,
    pub since: i64,
}

#[derive(Debug, Clone)]
struct RoostSync {
    store: Arc<Store>,
    state: Arc<Mutex<RoostState>>,
}

impl iroh::protocol::ProtocolHandler for RoostSync {
    async fn accept(
        &self,
        conn: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        // The door check: only members may pull history. The caller's identity is
        // authenticated by the transport, so it cannot be spoofed by a client.
        let who = conn.remote_id();
        if !self.state.lock().unwrap().perms.is_active_member(&who) {
            starling::logger::warn(&format!("roost-sync: refused non-member {who}"));
            return Ok(());
        }
        let Ok((mut send, mut recv)) = conn.accept_bi().await else {
            starling::logger::warn("roost-sync: failed to accept bi stream");
            return Ok(());
        };

        let Ok(req) = recv.read_to_end(256).await else {
            starling::logger::warn("roost-sync: failed to read request");
            return Ok(());
        };
        let Ok(request): Result<RoostSyncRequest, _> = postcard::from_bytes(&req) else {
            starling::logger::warn("roost-sync: invalid request format");
            return Ok(());
        };

        let history = match self.store.since(&request.channel, request.since) {
            Ok(history) => history,
            Err(e) => {
                starling::logger::warn(&format!("roost-sync: invalid request: {e}"));
                return Ok(());
            }
        };
        match postcard::to_stdvec(&history) {
            Ok(bytes) => {
                if bytes.len() > MAX_ROOST_SYNC_RESPONSE_BYTES {
                    starling::logger::warn(&format!(
                        "roost-sync: response for #{} is {} bytes, exceeds limit {}",
                        request.channel,
                        bytes.len(),
                        MAX_ROOST_SYNC_RESPONSE_BYTES
                    ));
                } else if let Err(e) = send.write_all(&bytes).await {
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

/// ALPN for the moderation protocol: ban/kick/invite/delete, each re-checked
/// roost-side against the sender's authenticated identity.
pub const MOD_ALPN: &[u8] = b"starling/mod/0";

#[derive(Debug, Clone)]
struct ModProto {
    state: Arc<Mutex<RoostState>>,
    store: Arc<Store>,
}

impl iroh::protocol::ProtocolHandler for ModProto {
    async fn accept(
        &self,
        conn: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        let from = conn.remote_id();
        if !self.state.lock().unwrap().perms.is_active_member(&from) {
            return Ok(());
        }
        let Ok((mut send, mut recv)) = conn.accept_bi().await else {
            return Ok(());
        };
        let Ok(bytes) = recv.read_to_end(1024).await else {
            return Ok(());
        };
        let Ok(req) = postcard::from_bytes::<ModRequest>(&bytes) else {
            return Ok(());
        };

        // Compute the verdict under the lock, then drop it before any await.
        // Compute the verdict under the lock, then drop it before any await. On a
        // successful mutation we snapshot perms and persist them so the roost's
        // member list, invitations, and bans survive a restart.
        let (verdict, dirty): (Result<(), String>, bool) = {
            let mut st = self.state.lock().unwrap();
            match req {
                ModRequest::Ban(target) => {
                    let r = st.perms.handle_ban(&from, &target);
                    let ok = r.is_ok();
                    (r.map_err(|e| e.to_string()), ok)
                }
                ModRequest::Kick(target) => {
                    let r = st.perms.handle_kick(&from, &target);
                    let ok = r.is_ok();
                    (r.map_err(|e| e.to_string()), ok)
                }
                ModRequest::Invite(target) => {
                    let r = st.perms.handle_invite(&from, target);
                    let ok = r.is_ok();
                    (r.map_err(|e| e.to_string()), ok)
                }
                ModRequest::DeleteMessage { channel, id } => {
                    let allowed = st.perms.effective(&from).contains(Perm::MANAGE_MSGS);
                    if !allowed {
                        (Err("not allowed".into()), false)
                    } else {
                        match self.store.delete_message(&channel, &id) {
                            Ok(true) => (Ok(()), false),
                            Ok(false) => (Err("message not found".into()), false),
                            Err(e) => (Err(e.to_string()), false),
                        }
                    }
                }
            }
        };
        if dirty {
            let snapshot = self.state.lock().unwrap().perms.clone();
            if let Err(e) = self.store.save_perms(&snapshot) {
                starling::logger::warn(&format!("roost: failed to persist perms: {e}"));
            }
        }

        let verdict = verdict;

        let _ = send
            .write_all(&postcard::to_stdvec(&verdict).unwrap_or_default())
            .await;
        let _ = send.finish();
        conn.closed().await;
        Ok(())
    }
}

/// ALPN for the join handshake: the only way to receive channel secrets. The
/// roost authenticates the caller's identity from the transport, runs the door
/// check, and on success returns the welcome (name + per-channel keys).
pub const JOIN_ALPN: &[u8] = b"starling/roost-join/0";
const MAX_JOIN_RESPONSE_BYTES: usize = 65_536;

#[derive(Debug, Clone)]
struct JoinProto {
    state: Arc<Mutex<RoostState>>,
    store: Arc<Store>,
}

impl iroh::protocol::ProtocolHandler for JoinProto {
    async fn accept(
        &self,
        conn: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        let who = conn.remote_id();
        let Ok(mut send) = conn.open_uni().await else {
            return Ok(());
        };

        // The door check: invited birds become members on first join; banned
        // birds and uninvited strangers are refused.
        //
        // On success, the welcome carries per-channel secrets AND the control
        // channel secret. All three are high-entropy random values minted by
        // the store; none are derivable from the public roost code.
        let verdict: Result<RoostWelcome, String> = {
            let mut st = self.state.lock().unwrap();
            match st.perms.handle_join(&who) {
                Ok(()) => {
                    // Persist the updated membership so it survives a restart.
                    let snapshot = st.perms.clone();
                    let channels = st
                        .channels
                        .iter()
                        .filter_map(|c| Some((c.clone(), self.store.channel_secret(c).ok()?)))
                        .collect();
                    let control_secret = self.store.control_secret().ok();
                    if control_secret.is_none() {
                        starling::logger::warn(&format!(
                            "roost-join: control secret unavailable for {who}"
                        ));
                    }
                    if let Err(e) = self.store.save_perms(&snapshot) {
                        starling::logger::warn(&format!(
                            "roost: failed to persist perms after join: {e}"
                        ));
                    }
                    Ok(RoostWelcome {
                        name: st.name.clone(),
                        channels,
                        control_secret,
                    })
                }
                Err(e) => Err(e.to_string()),
            }
        };

        let encoded = postcard::to_stdvec(&verdict).unwrap_or_default();
        if encoded.len() <= MAX_JOIN_RESPONSE_BYTES {
            let _ = send.write_all(&encoded).await;
        } else {
            starling::logger::warn(&format!(
                "roost-join: welcome for {who} is {} bytes, exceeds limit",
                encoded.len()
            ));
        }
        let _ = send.finish();
        conn.closed().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::validate_roost_name;

    #[test]
    fn validates_roost_names_before_building_paths() {
        for valid in ["starling", "my-roost_2", "A"] {
            assert!(validate_roost_name(valid).is_ok());
        }
        for invalid in ["", ".", "..", "../outside", "a/b", "a\\b", "with space"] {
            assert!(
                validate_roost_name(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }
}
