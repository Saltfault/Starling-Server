//! Voice call layer: opens a direct QUIC connection to a peer and streams
//! Opus frames as QUIC datagrams.
//!
//! Outgoing calls use [`place_call`]; incoming calls are handled by
//! [`handle_incoming`], which is invoked by `VoiceProto` in [`crate::net`].

use crate::event::AppEvent;
use iroh::{Endpoint, EndpointAddr, endpoint::Connection};
use tokio::sync::mpsc;

/// ALPN string for the voice protocol. Both sides must agree on this.
pub const VOICE_ALPN: &[u8] = b"starling/voice/0";

/// Place an outgoing call: connect to `peer` and stream mic frames as QUIC
/// datagrams until the mic channel is closed (i.e. the caller hangs up).
///
/// This is spawned as a background task by [`crate::net::run`].
pub async fn place_call(
    endpoint: Endpoint,
    peer: EndpointAddr,
    mut frame_rx: mpsc::UnboundedReceiver<Vec<u8>>,
) -> anyhow::Result<()> {
    let conn = endpoint.connect(peer, VOICE_ALPN).await?;

    // Each 20 ms Opus frame is sent as a single QUIC datagram.
    // The loop ends when `frame_rx` is closed (mic stream dropped on hang-up).
    while let Some(frame) = frame_rx.recv().await {
        let _ = conn.send_datagram(frame.into());
    }

    Ok(())
}

/// Handle an incoming call: read datagrams from the connection and forward
/// them to the UI as [`AppEvent::VoiceFrame`].
///
/// Called by `VoiceProto::accept` in [`crate::net`].
pub async fn handle_incoming(
    conn: Connection,
    evt_tx: mpsc::UnboundedSender<AppEvent>,
) -> anyhow::Result<()> {
    // Loop until the remote peer hangs up (connection closes).
    while let Ok(bytes) = conn.read_datagram().await {
        let _ = evt_tx.send(AppEvent::VoiceFrame(bytes.to_vec()));
    }

    Ok(())
}
