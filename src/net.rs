use starling::crypto::FlockCrypto;
use starling::event::{BirdStatus, ChatMessage, GossipPayload};
use crate::event::{AppEvent, Command};
use iroh::{
    Endpoint, EndpointId,
    endpoint::{Connection, presets},
    protocol::Router,
};
use iroh_gossip::{
    api::Event,
    net::{GOSSIP_ALPN, Gossip},
};
use n0_future::StreamExt;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;

pub async fn run(
    bootstrap: Vec<EndpointId>,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    evt_tx: mpsc::UnboundedSender<AppEvent>,
    muted: Arc<AtomicBool>,
    name: String,
    input_device: Option<String>,
) -> anyhow::Result<()> {
    let secret = starling::config::Profile::load_or_create_secret();
    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret)
        .bind()
        .await?;
    endpoint.online().await;

    let my_node_id = endpoint.addr().id;
    let opener_id = bootstrap.first().copied().unwrap_or(my_node_id);

    let room_code = starling::net::encode_node_id(&opener_id);
    let topic = starling::net::topic_for(&format!("starling/flock/{room_code}"));
    let crypto = FlockCrypto::from_room_code(&room_code);

    let my_code = starling::net::encode_node_id(&my_node_id);
    starling::logger::warn(&format!("endpoint bound: room_code={my_code}"));
    let _ = evt_tx.send(AppEvent::Ticket(my_code));

    let gossip = Gossip::builder().spawn(endpoint.clone());

    let history: starling::sync::History = Default::default();

    let _router = Router::builder(endpoint.clone())
        .accept(GOSSIP_ALPN, gossip.clone())
        .accept(
            crate::call::VOICE_ALPN,
            VoiceProto {
                evt_tx: evt_tx.clone(),
            },
        )
        .accept(
            crate::call::VIDEO_ALPN,
            VideoProto {
                evt_tx: evt_tx.clone(),
            },
        )
        .accept(
            starling::sync::SYNC_ALPN,
            starling::sync::SyncProto {
                history: history.clone(),
            },
        )
        .spawn();

    let (sender, mut receiver) = gossip.subscribe(topic, bootstrap).await?.split();

    if opener_id != my_node_id {
        let (ep, tx) = (endpoint.clone(), evt_tx.clone());
        tokio::spawn(async move {
            let _ = crate::sync::backfill(ep, opener_id, 0, tx).await;
        });
    }

    let mut _mic_stream: Option<cpal::Stream> = None;
    let mut _cam_thread: Option<std::thread::JoinHandle<()>> = None;

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
                    starling::net::broadcast_payload(&sender, &crypto, &GossipPayload::Chat(msg.clone())).await?;
                    let _ = evt_tx.send(AppEvent::Message(msg.clone()));
                    history.lock().unwrap().push(msg);
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
                    starling::net::broadcast_payload(&sender, &crypto, &GossipPayload::Status {
                        id: my_node_id, status: BirdStatus::InCall,
                    }).await?;
                }

                Command::HangUp => {
                    _mic_stream = None;
                    starling::net::broadcast_payload(&sender, &crypto, &GossipPayload::Status {
                        id: my_node_id, status: BirdStatus::Online,
                    }).await?;
                }

                Command::StartVideo(addr) => {
                    let (cam_tx, cam_rx) = mpsc::unbounded_channel();
                    _cam_thread = Some(crate::video::start_camera(cam_tx)?);
                    let ep = endpoint.clone();
                    tokio::spawn(async move {
                        let _ = crate::call::place_video(ep, addr, cam_rx).await;
                    });
                }
                Command::StopVideo => { _cam_thread = None; }

                Command::Quit => break,
            },

            Some(event) = receiver.next() => {
                match event {
                    Ok(Event::Received(msg)) => {
                        if let Some(plaintext) = crypto.decrypt(&msg.content) {
                            match postcard::from_bytes::<GossipPayload>(&plaintext) {
                                Ok(GossipPayload::Chat(m)) => {
                                    history.lock().unwrap().push(m.clone());
                                    let _ = evt_tx.send(AppEvent::Message(m));
                                }
                                Ok(GossipPayload::Profile { id, name }) => {
                                    let _ = evt_tx.send(AppEvent::PeerNamed(id, name));
                                }
                                Ok(GossipPayload::Status { id, status }) => {
                                    let _ = evt_tx.send(AppEvent::PeerStatus(id, status));
                                }
                                Err(e) => {
                                    starling::logger::error(&format!("gossip deserialize error: {e}"));
                                }
                            }
                        }
                    }
                    Ok(Event::NeighborUp(id)) => {
                        starling::logger::warn(&format!("neighbor up: {}", id));
                        let _ = evt_tx.send(AppEvent::PeerConnected(id));
                        let payload = GossipPayload::Profile {
                            id: my_node_id,
                            name: name.clone(),
                        };
                        if let Err(e) = starling::net::broadcast_payload(&sender, &crypto, &payload).await {
                            starling::logger::error(&format!("profile broadcast failed: {e}"));
                        }
                    }
                    Ok(Event::NeighborDown(id)) => {
                        starling::logger::warn(&format!("neighbor down: {}", id));
                        let _ = evt_tx.send(AppEvent::PeerDisconnected(id));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        starling::logger::error(&format!("gossip stream error: {e}"));
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

#[derive(Debug)]
struct VideoProto {
    evt_tx: mpsc::UnboundedSender<AppEvent>,
}

impl iroh::protocol::ProtocolHandler for VideoProto {
    async fn accept(&self, conn: Connection) -> Result<(), iroh::protocol::AcceptError> {
        let _ = crate::call::recv_video(conn, self.evt_tx.clone()).await;
        Ok(())
    }
}
