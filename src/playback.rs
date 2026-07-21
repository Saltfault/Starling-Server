// TODO: real playback. For now this is a stub so main.rs compiles.
// A full implementation will decode the opus bytes to PCM samples and push
// them into a ring buffer that a cpal output stream drains.

pub struct Playback;

impl Playback {
    pub fn new() -> Self {
        Self
    }

    pub fn push_opus(&mut self, _bytes: &[u8]) {
        // TODO: opus-decode `_bytes` to f32 samples and push into the ring buffer
    }
}
