//! History backfill: a joining bird asks a peer for recent messages.

use crate::event::{AppEvent, ChatMessage};
use iroh::{Endpoint, EndpointAddr, EndpointId, endpoint::Connection};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub const SYNC_ALPN: &[u8] = b"starling/sync/0";
const MAX_MESSAGES: usize = 500;

/// Shared scrollback, owned by net.rs, served by SyncProto.
pub type History = Arc<Mutex<Vec<ChatMessage>>>;

/// SERVER side: answer one backfill request on an incoming connection.
#[derive(Debug, Clone)]
pub struct SyncProto {
    pub history: History,
}

impl iroh::protocol::ProtocolHandler for SyncProto {
    async fn accept(&self, conn: Connection) -> Result<(), iroh::protocol::AcceptError> {
        let _ = self.serve(conn).await; // errors logged, never fatal
        Ok(())
    }
}

impl SyncProto {
    async fn serve(&self, conn: Connection) -> anyhow::Result<()> {
        let (mut send, mut recv) = conn.accept_bi().await?;
        let req = recv.read_to_end(64).await?;
        let since: i64 = postcard::from_bytes(&req)?;

        let recent: Vec<ChatMessage> = {
            let h = self.history.lock().unwrap();
            let mut filtered: Vec<_> = h.iter().filter(|m| m.ts > since).cloned().collect();
            // Keep only the newest MAX_MESSAGES, in chronological order.
            if filtered.len() > MAX_MESSAGES {
                filtered = filtered.split_off(filtered.len() - MAX_MESSAGES);
            }
            filtered
        }; // lock dropped before any await

        send.write_all(&postcard::to_stdvec(&recent)?).await?;
        send.finish()?;
        // let the client read everything before the connection drops
        conn.closed().await;
        Ok(())
    }
}

/// CLIENT side: called once after joining, pointed at the bootstrap bird.
pub async fn backfill(
    endpoint: Endpoint,
    peer: EndpointId,
    since: i64,
    evt_tx: mpsc::UnboundedSender<AppEvent>,
) -> anyhow::Result<()> {
    let conn = endpoint
        .connect(EndpointAddr::from(peer), SYNC_ALPN)
        .await?;
    let (mut send, mut recv) = conn.open_bi().await?;
    send.write_all(&postcard::to_stdvec(&since)?).await?;
    send.finish()?;
    let bytes = recv.read_to_end(10_000_000).await?;
    let messages: Vec<ChatMessage> = postcard::from_bytes(&bytes)?;
    if !messages.is_empty() {
        let _ = evt_tx.send(AppEvent::HistoryChunk(messages));
    }
    Ok(())
}
