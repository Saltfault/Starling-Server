//! Starling Server — the shared protocol library used by all Starling clients.
//!
//! This crate contains everything that talks to the murmuration:
//! networking, cryptography, voice/video pipelines, and protocol handlers.
//! It has no UI code — clients (TUI, Desktop, Android, Web) depend on this
//! library and provide their own UI layer.

pub mod call;
pub mod config;
pub mod crypto;
pub mod event;
pub mod logger;
pub mod net;
pub mod opus_ffi;
pub mod playback;
pub mod roost;
pub mod sync;
pub mod util;
pub mod video;
pub mod voice;
