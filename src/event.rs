use serde::{Deserialize, Serialize};

// UI -> Network, things the user does
pub enum Command {
    SendText(String),
    Quit,
}

// Network -> UI, things that happen
pub enum AppEvent {
    Message(ChatMessage),
    PeerConnected(String),
    PeerDisconnected(String),
    Ticket(String), // the shareable invite, emmited once bound
}

// The actual chat message that travels via gossip
#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,     // uuid
    pub author: String, // user display name
    pub body: String,
    pub ts: i64,
}
