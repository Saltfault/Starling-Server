use crate::event::{AppEvent, ChatMessage, Command};
use iroh::{Endpoint, EndpointId, endpoint::presets, protocol::Router};
use iroh_gossip::{
    api::Event,
    net::{GOSSIP_ALPN, Gossip},
    proto::TopicId,
};
use iroh_tickets::endpoint::EndpointTicket;
use n0_future::StreamExt;
use tokio::sync::mpsc;

// Derive a stabe 32 byte topic id from a human name
pub fn topic_for(name: &str) -> TopicId {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(name.as_bytes());
    TopicId::from_bytes(hash.into())
}

// Spawned once. Owns the endpoint and gossip, bridges the two channels.
// 'bootstrap' are peers pulled from a join ticket (empty if we are the one who opened it)
pub async fn run(
    topic: TopicId,
    bootstrap: Vec<EndpointId>,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    evt_tx: mpsc::UnboundedSender<AppEvent>,
) -> anyhow::Result<()> {
    // bind a QUIC endpoint with n0's relay and discovery presets
    let endpoint = Endpoint::bind(presets::N0).await?;
    endpoint.online().await; // wait for a home relay

    let gossip = Gossip::builder().spawn(endpoint.clone());
    let _router = Router::builder(endpoint.clone())
        .accept(GOSSIP_ALPN, gossip.clone())
        .spawn();

    // hand the UI a ticket others cna use to join us
    let ticket = EndpointTicket::new(endpoint.addr());
    let _ = evt_tx.send(AppEvent::Ticket(ticket.to_string()));

    let (sender, mut receiver) = gossip.subscribe_and_join(topic, bootstrap).await?.split();

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => match cmd {
                Command::SendText(text) => {
                    let msg = ChatMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        author: whoami(),
                        body: text,
                        ts: chrono::Utc::now().timestamp_millis(),
                    };
                    let bytes = postcard::to_stdvec(&msg)?;
                    sender.broadcast(bytes.into()).await?;
                    let _ = evt_tx.send(AppEvent::Message(msg));
                }

                Command::Quit => break,
            },

            Some(event) = receiver.next() => match event? {
                Event::Received(msg) => {
                    if let Ok(m) = postcard::from_bytes::<ChatMessage>(&msg.content) {
                        let _ = evt_tx.send(AppEvent::Message(m));
                    }
                }

                Event::NeighborUp(id) =>
                    { let _ = evt_tx.send(AppEvent::PeerConnected(id.to_string())); }
                Event::NeighborDown(id) =>
                    { let _ = evt_tx.send(AppEvent::PeerDisconnected(id.to_string())); }
                _ => {}
            }
        }
    }

    Ok(())
}

fn whoami() -> String {
    std::env::var("STARLING_NAME").unwrap_or_else(|_| "anon".into())
}
