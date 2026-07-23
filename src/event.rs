use iroh::EndpointAddr;
use iroh::EndpointId;

pub enum Command {
    SendText(String),
    StartCall(EndpointAddr),
    HangUp,
    StartVideo(EndpointAddr),
    StopVideo,
    Quit,
}

#[derive(Debug)]
pub enum AppEvent {
    Message(starling::event::ChatMessage),
    PeerConnected(EndpointId),
    PeerDisconnected(EndpointId),
    PeerNamed(EndpointId, String),
    Ticket(String),
    VoiceFrame(Vec<u8>),
    VideoFrame(Vec<u8>),
    PeerStatus(EndpointId, starling::event::BirdStatus),
    HistoryChunk(Vec<starling::event::ChatMessage>),
}
