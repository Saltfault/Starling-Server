use iroh::EndpointAddr;
use serde::{Deserialize, Serialize};

// UI -> Network, things the user does
pub enum Command {
    SendText(String),
    StartCall(EndpointAddr),
    HangUp,
    Quit,
}

// Network -> UI, things that happen
#[derive(Debug)]
pub enum AppEvent {
    Message(ChatMessage),
    PeerConnected(String),
    PeerDisconnected(String),
    Ticket(String), // the shareable invite, emmited once bound
    VoiceFrame(Vec<u8>),
}

// The actual chat message that travels via gossip
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,     // uuid
    pub author: String, // user display name
    pub body: String,
    pub ts: i64,
}
