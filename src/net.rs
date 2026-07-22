//! Network layer: owns the iroh [`Endpoint`], the gossip subscription, and
//! the voice protocol handler. Bridges the UI ↔ network channels.
//!
//! All gossip text messages are **end-to-end encrypted** with
//! ChaCha20-Poly1305 using a key derived from the room code. Voice calls are
//! E2E encrypted via iroh's QUIC TLS 1.3.

use crate::crypto::FlockCrypto;
use crate::event::{AppEvent, ChatMessage, Command};
use iroh::{
    Endpoint, EndpointId,
    endpoint::{Connection, presets},
    protocol::Router,
};
use iroh_gossip::{
    api::Event,
    net::{GOSSIP_ALPN, Gossip},
    proto::TopicId,
};
use n0_future::StreamExt;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;

/// Derive a stable 32-byte [`TopicId`] from a human-readable name by hashing
/// it with SHA-256.
pub fn topic_for(name: &str) -> TopicId {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(name.as_bytes());
    TopicId::from_bytes(hash.into())
}

/// Derive a short room code from a node ID: "BIRD" + first 6 hex chars.
/// This is the first color group of the full invite code.
pub fn room_code_from_node_id(node_id: &EndpointId) -> String {
    let bytes = node_id.as_bytes();
    let hex: String = (0..3).map(|i| format!("{:02X}", bytes[i])).collect();
    format!("BIRD{hex}")
}

/// Encode a 32-byte node ID as a string of hex color codes:
/// `BIRD-RRGGBB-RRGGBB-...` (11 groups, last group zero-padded).
pub fn encode_node_id(node_id: &EndpointId) -> String {
    let bytes = node_id.as_bytes();
    // Pad to a multiple of 3 so every group is a full RGB triple.
    let mut padded = bytes.to_vec();
    while padded.len() % 3 != 0 {
        padded.push(0);
    }
    let colors: Vec<String> = padded
        .chunks(3)
        .map(|c| format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
        .collect();
    format!("BIRD-{}", colors.join("-"))
}

/// Decode a `BIRD-RRGGBB-RRGGBB-...` string back into an EndpointId.
pub fn decode_node_id(code: &str) -> Option<EndpointId> {
    let code = code
        .strip_prefix("BIRD-")
        .or_else(|| code.strip_prefix("BIRD"))?;
    let mut bytes = Vec::new();
    for group in code.split('-') {
        if group.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&group[0..2], 16).ok()?;
        let g = u8::from_str_radix(&group[2..4], 16).ok()?;
        let b = u8::from_str_radix(&group[4..6], 16).ok()?;
        bytes.push(r);
        bytes.push(g);
        bytes.push(b);
    }
    if bytes.len() < 32 {
        return None;
    }
    let arr: [u8; 32] = bytes[..32].try_into().ok()?;
    EndpointId::from_bytes(&arr).ok()
}

/// The main network loop. Spawned once by `main`.
pub async fn run(
    bootstrap: Vec<EndpointId>,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    evt_tx: mpsc::UnboundedSender<AppEvent>,
    muted: Arc<AtomicBool>,
    name: String,
    input_device: Option<String>,
) -> anyhow::Result<()> {
    crate::logger::warn("binding endpoint...");

    let endpoint = Endpoint::bind(presets::N0).await?;
    endpoint.online().await;

    let my_node_id = endpoint.addr().id;
    let opener_id = bootstrap.first().copied().unwrap_or(my_node_id);
    let room_code = room_code_from_node_id(&opener_id);
    let topic = topic_for(&format!("starling/flock/{room_code}"));
    let crypto = FlockCrypto::from_room_code(&room_code);

    let ticket = encode_node_id(&my_node_id);
    crate::logger::warn(&format!(
        "endpoint bound: room_code={} ticket={}",
        room_code, ticket
    ));
    let _ = evt_tx.send(AppEvent::Ticket(ticket));

    let gossip = Gossip::builder().spawn(endpoint.clone());

    let _router = Router::builder(endpoint.clone())
        .accept(GOSSIP_ALPN, gossip.clone())
        .accept(
            crate::call::VOICE_ALPN,
            VoiceProto {
                evt_tx: evt_tx.clone(),
            },
        )
        .spawn();

    let (sender, mut receiver) = gossip.subscribe(topic, bootstrap).await?.split();

    crate::logger::warn("subscribed to gossip topic");

    let mut _mic_stream: Option<cpal::Stream> = None;

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => match cmd {
                Command::SendText(text) => {
                    let msg = ChatMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        author: name.clone(),
                        body: text,
                        ts: chrono::Utc::now().timestamp_millis(),
                    };
                    let plaintext = postcard::to_stdvec(&msg)?;
                    let ciphertext = crypto.encrypt(&plaintext);
                    sender.broadcast(ciphertext.into()).await?;
                    let _ = evt_tx.send(AppEvent::Message(msg));
                }

                Command::StartCall(addr) => {
                    let (mic_tx, mic_rx) = mpsc::unbounded_channel();
                    _mic_stream = Some(crate::voice::start_capture(
                        mic_tx, muted.clone(), input_device.as_deref(),
                    )?);
                    let ep = endpoint.clone();
                    tokio::spawn(async move {
                        let _ = crate::call::place_call(ep, addr, mic_rx).await;
                    });
                }

                Command::HangUp => { _mic_stream = None; }

                Command::Quit => break,
            },

            Some(event) = receiver.next() => {
                match event {
                    Ok(Event::Received(msg)) => {
                        if let Some(plaintext) = crypto.decrypt(&msg.content) {
                            if let Ok(m) = postcard::from_bytes::<ChatMessage>(&plaintext) {
                                let _ = evt_tx.send(AppEvent::Message(m));
                            }
                        }
                    }
                    Ok(Event::NeighborUp(id)) => {
                        crate::logger::warn(&format!("neighbor up: {}", id));
                        let _ = evt_tx.send(AppEvent::PeerConnected(id));
                    }
                    Ok(Event::NeighborDown(id)) => {
                        crate::logger::warn(&format!("neighbor down: {}", id));
                        let _ = evt_tx.send(AppEvent::PeerDisconnected(id));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        crate::logger::error(&format!("gossip stream error: {e}"));
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct VoiceProto {
    evt_tx: mpsc::UnboundedSender<AppEvent>,
}

impl iroh::protocol::ProtocolHandler for VoiceProto {
    async fn accept(&self, conn: Connection) -> Result<(), iroh::protocol::AcceptError> {
        let _ = crate::call::handle_incoming(conn, self.evt_tx.clone()).await;
        Ok(())
    }
}
