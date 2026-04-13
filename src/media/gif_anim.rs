use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, Delay, DynamicImage};
use std::io::Cursor;
use std::time::Duration;

pub fn is_gif_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 6 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"))
}

fn delay_to_duration(delay: Delay) -> Duration {
    let (n, d) = delay.numer_denom_ms();
    if d == 0 || n == 0 {
        return Duration::from_millis(100);
    }
    let ms = (n as f64 / d as f64).round().clamp(20.0, 30_000.0) as u64;
    Duration::from_millis(ms)
}

const MAX_FRAME_DIM: u32 = 256;

pub fn decode_gif_animation(bytes: &[u8]) -> Option<(Vec<DynamicImage>, Vec<Duration>)> {
    if !is_gif_bytes(bytes) {
        return None;
    }
    let decoder = GifDecoder::new(Cursor::new(bytes)).ok()?;
    let raw = decoder.into_frames().collect_frames().ok()?;
    if raw.len() <= 1 {
        return None;
    }

    const MAX_FRAMES: usize = 200;
    const MAX_PIXELS: u64 = 1024 * 1024;

    let take = raw.len().min(MAX_FRAMES);
    let mut frames = Vec::with_capacity(take);
    let mut delays = Vec::with_capacity(take);

    for f in raw.into_iter().take(take) {
        let delay = delay_to_duration(f.delay());
        let buf = f.into_buffer();
        let px = buf.width() as u64 * buf.height() as u64;
        if px > MAX_PIXELS {
            return None;
        }
        let img = DynamicImage::ImageRgba8(buf);
        let img = shrink_frame(img);
        delays.push(delay);
        frames.push(img);
    }

    if frames.len() <= 1 {
        return None;
    }

    Some((frames, delays))
}

fn shrink_frame(img: DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w <= MAX_FRAME_DIM && h <= MAX_FRAME_DIM {
        return img;
    }
    let scale = (MAX_FRAME_DIM as f64 / w as f64).min(MAX_FRAME_DIM as f64 / h as f64);
    let nw = ((w as f64 * scale).round() as u32).max(1);
    let nh = ((h as f64 * scale).round() as u32).max(1);
    img.resize_exact(nw, nh, image::imageops::FilterType::Nearest)
}
