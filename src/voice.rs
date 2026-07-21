use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Application, Channels, Encoder};
use tokio::sync::mpsc;

// 48kHz mono, 20ms frames = 960 samples/frame (Opus standard)
const SAMPLE_RATE: u32 = 48_000;
const FRAME: usize = 960;

pub fn start_capture(net_tx: mpsc::UnboundedSender<Vec<u8>>) -> anyhow::Result<cpal::Stream> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("no mic"))?;
    let cfg = cpal::StreamConfig {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Default,
    };
    let mut enc = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip)?;
    let mut acc: Vec<f32> = Vec::with_capacity(FRAME);

    let stream = device.build_input_stream(
        cfg,
        move |data: &[f32], _| {
            acc.extend_from_slice(data);
            while acc.len() >= FRAME {
                let frame: Vec<f32> = acc.drain(..FRAME).collect();
                let mut out = vec![0u8; 400];
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
