//! Microphone capture: reads from the default input device, encodes 20 ms
//! frames with Opus, and sends the compressed bytes over an mpsc channel.
//!
//! The capture stream runs on the audio thread. Mute state is shared via an
//! `Arc<AtomicBool>` so the UI can toggle it without crossing thread
//! boundaries unsafely.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Application, Channels, Encoder};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

/// Sample rate: 48 kHz (Opus standard for VoIP).
const SAMPLE_RATE: u32 = 48_000;

/// Frame size: 960 samples = 20 ms at 48 kHz.
const FRAME: usize = 960;

/// Start microphone capture.
///
/// Returns a [`cpal::Stream`] that **must be kept alive** for the duration of
/// the call — dropping it stops the mic.
///
/// * `net_tx` — receives encoded Opus frames (one `Vec<u8>` per 20 ms frame).
/// * `muted` — checked on every frame. When `true`, audio is still read from
///   the device (to keep the stream healthy) but frames are discarded instead
///   of being encoded and sent.
pub fn start_capture(
    net_tx: mpsc::UnboundedSender<Vec<u8>>,
    muted: Arc<AtomicBool>,
) -> anyhow::Result<cpal::Stream> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("no microphone input device found"))?;

    let cfg = cpal::StreamConfig {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Default,
    };

    let mut enc = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip)?;

    // Accumulator for incoming samples until we have a full Opus frame.
    let mut acc: Vec<f32> = Vec::with_capacity(FRAME);

    let stream = device.build_input_stream(
        cfg,
        move |data: &[f32], _: &_| {
            acc.extend_from_slice(data);

            // Encode and send as many complete frames as we have accumulated.
            while acc.len() >= FRAME {
                let frame: Vec<f32> = acc.drain(..FRAME).collect();

                // If muted, skip encoding — just discard the frame.
                if muted.load(Ordering::Relaxed) {
                    continue;
                }

                let mut out = vec![0u8; 400]; // 400 bytes is generous for Opus
                if let Ok(n) = enc.encode_float(&frame, &mut out) {
                    out.truncate(n);
                    let _ = net_tx.send(out);
                }
            }
        },
        |e| eprintln!("mic error: {e}"),
        None,
    )?;

    stream.play()?;
    Ok(stream)
}
