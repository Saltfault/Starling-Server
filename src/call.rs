use crate::event::AppEvent;
use iroh::{Endpoint, EndpointAddr, endpoint::Connection};
use tokio::sync::mpsc;

pub const VOICE_ALPN: &[u8] = b"starling/voice/0";

pub async fn place_call(
    endpoint: Endpoint,
    peer: EndpointAddr,
    mut frame_rx: mpsc::UnboundedReceiver<Vec<u8>>,
) -> anyhow::Result<()> {
    let conn = endpoint.connect(peer, VOICE_ALPN).await?;
    while let Some(frame) = frame_rx.recv().await {
        let _ = conn.send_datagram(frame.into());
    }

    Ok(())
}

pub async fn handle_incoming(
    conn: Connection,
    evt_tx: mpsc::UnboundedSender<AppEvent>,
) -> anyhow::Result<()> {
    while let Ok(bytes) = conn.read_datagram().await {
        let _ = evt_tx.send(AppEvent::VoiceFrame(bytes.to_vec()));
    }

    Ok(())
}
