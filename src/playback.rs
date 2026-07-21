//! Audio playback: decodes incoming Opus frames and plays them through a cpal
//! output stream.
//!
//! A lock-free ring buffer ([`ringbuf::SharedRb`]) bridges two threads:
//!
//! * **Producer** (main thread) — `push_opus` decodes an Opus frame into PCM
//!   samples and writes them into the ring buffer.
//! * **Consumer** (audio output thread) — the cpal callback drains the ring
//!   buffer in real time. If the buffer is empty (underrun), it outputs
//!   silence.
//!
//! The output stream is stored in the [`Playback`] struct so it lives as long
//! as the struct does. Dropping `Playback` stops audio output.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Channels, Decoder};
use ringbuf::{CachingCons, CachingProd, SharedRb, storage::Heap, traits::*};

/// Sample rate: 48 kHz (must match the encoder in [`crate::voice`]).
const SAMPLE_RATE: u32 = 48_000;

/// Frame size: 960 samples = 20 ms at 48 kHz.
const FRAME: usize = 960;

/// Ring buffer capacity: ~2 seconds of audio. Generous enough to absorb
/// jitter without underrunning, small enough to keep latency reasonable.
const BUFFER_CAPACITY: usize = SAMPLE_RATE as usize * 2;

/// Type alias for the producer half of the ring buffer.
type Prod = CachingProd<std::sync::Arc<SharedRb<Heap<f32>>>>;

/// Type alias for the consumer half of the ring buffer.
type Cons = CachingCons<std::sync::Arc<SharedRb<Heap<f32>>>>;

/// Audio playback engine. Owns the Opus decoder, ring buffer producer, and
/// the cpal output stream.
pub struct Playback {
    /// Opus decoder (mono, 48 kHz).
    decoder: Decoder,
    /// Writes decoded PCM samples into the ring buffer.
    producer: Prod,
    /// The cpal output stream. Kept alive in the struct; dropping it stops
    /// playback.
    _stream: cpal::Stream,
}

impl Playback {
    /// Set up the output stream and ring buffer.
    ///
    /// The stream starts immediately and plays silence until frames arrive.
    /// Returns an error if no output device is available.
    ///
    /// ALSA error spam is suppressed on Unix so that fallback failures don't
    /// corrupt the terminal UI.
    pub fn new() -> anyhow::Result<Self> {
        crate::util::suppress_stderr(Self::new_inner)
    }

    fn new_inner() -> anyhow::Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no audio output device found"))?;

        let cfg = cpal::StreamConfig {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Default,
        };

        // Split the ring buffer: producer stays here, consumer goes to the
        // audio callback.
        let rb = SharedRb::<Heap<f32>>::new(BUFFER_CAPACITY);
        let (producer, mut consumer): (Prod, Cons) = rb.split();

        let stream = device.build_output_stream(
            cfg,
            move |data: &mut [f32], _: &_| {
                // Pop decoded samples from the ring buffer.
                let n = consumer.pop_slice(data);
                // Fill any remaining slots with silence (underrun).
                for sample in &mut data[n..] {
                    *sample = 0.0;
                }
            },
            |e| eprintln!("playback error: {e}"),
            None,
        )?;

        stream.play()?;

        let decoder = Decoder::new(SAMPLE_RATE, Channels::Mono)?;

        Ok(Self {
            decoder,
            producer,
            _stream: stream,
        })
    }

    /// Decode an Opus frame and push the resulting PCM samples into the ring
    /// buffer.
    ///
    /// If the buffer is full the excess samples are silently dropped (the
    /// oldest data is already being played, so we prefer to drop new data
    /// rather than glitch the output).
    pub fn push_opus(&mut self, bytes: &[u8]) {
        let mut pcm = [0f32; FRAME];
        match self.decoder.decode_float(bytes, &mut pcm, false) {
            Ok(n) => {
                self.producer.push_slice(&pcm[..n]);
            }
            Err(e) => eprintln!("opus decode error: {e}"),
        }
    }
}
