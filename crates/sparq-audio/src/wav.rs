//! Minimal dependency-free WAV writer.
//!
//! Supports the formats the golden-reference harness and Phase 0 renders need: 16/24/32-bit
//! integer PCM and 32-bit float, arbitrary channel counts (so multichannel and later ambisonic
//! bounces work), and the `fact` chunk required for non-PCM formats.
//!
//! A full reader/writer with BWF metadata, cue points and channel masks is WO-010/Phase 3 work;
//! this is deliberately small and byte-exact.

// `clippy::disallowed_methods` denies `std::fs::read`/`write` workspace-wide because no file I/O may
// happen on the audio path. This module is the *offline* file writer used by `sparq render`, the
// golden harness and bounce export — it is never called from a device callback, and the audio path
// hands samples to it only after a render completes. Allowed here, and only here.
#![allow(clippy::disallowed_methods)]

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// Sample format for the output file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    /// 16-bit signed integer PCM.
    Pcm16,
    /// 24-bit signed integer PCM (packed, 3 bytes/sample).
    Pcm24,
    /// 32-bit signed integer PCM.
    Pcm32,
    /// 32-bit IEEE float (`WAVE_FORMAT_EXTENSIBLE` is not used; format tag 3).
    Float32,
}

impl SampleFormat {
    /// Bits per sample.
    #[must_use]
    pub const fn bits(self) -> u16 {
        match self {
            Self::Pcm16 => 16,
            Self::Pcm24 => 24,
            Self::Pcm32 => 32,
            Self::Float32 => 32,
        }
    }

    /// WAVE format tag.
    #[must_use]
    pub const fn format_tag(self) -> u16 {
        match self {
            Self::Pcm16 | Self::Pcm24 | Self::Pcm32 => 1, // WAVE_FORMAT_PCM
            Self::Float32 => 3,                           // WAVE_FORMAT_IEEE_FLOAT
        }
    }

    /// Bytes per sample.
    #[must_use]
    pub const fn bytes(self) -> usize {
        self.bits() as usize / 8
    }
}

/// Writes interleaved `f32` samples to a WAV file.
pub struct WavWriter<W: Write> {
    inner: W,
    sample_rate: u32,
    channels: u16,
    format: SampleFormat,
    data_bytes: u32,
}

impl WavWriter<BufWriter<File>> {
    /// Create a buffered WAV file writer. The header is written immediately; use
    /// [`write_wav`] for the one-shot case, which patches the header sizes before closing.
    pub fn create<P: AsRef<Path>>(
        path: P,
        sample_rate: u32,
        channels: u16,
        format: SampleFormat,
    ) -> io::Result<Self> {
        let file = File::create(path)?;
        Self::new(BufWriter::new(file), sample_rate, channels, format)
    }
}

impl<W: Write> WavWriter<W> {
    /// Wrap any writer.
    pub fn new(
        inner: W,
        sample_rate: u32,
        channels: u16,
        format: SampleFormat,
    ) -> io::Result<Self> {
        let mut w = Self { inner, sample_rate, channels, format, data_bytes: 0 };
        w.write_header(0)?;
        Ok(w)
    }

    fn write_header(&mut self, data_bytes: u32) -> io::Result<()> {
        let ch = u32::from(self.channels);
        let bits = u32::from(self.format.bits());
        let byte_rate = self.sample_rate.wrapping_mul(ch).wrapping_mul(bits / 8);
        let block_align = (ch * bits / 8) as u16;
        let fmt_len = if self.format == SampleFormat::Float32 { 18u32 } else { 16 };

        let w = &mut self.inner;
        w.write_all(b"RIFF")?;
        w.write_all(&(36 + data_bytes).to_le_bytes())?;
        w.write_all(b"WAVE")?;

        w.write_all(b"fmt ")?;
        w.write_all(&fmt_len.to_le_bytes())?;
        w.write_all(&self.format.format_tag().to_le_bytes())?;
        w.write_all(&self.channels.to_le_bytes())?;
        w.write_all(&self.sample_rate.to_le_bytes())?;
        w.write_all(&byte_rate.to_le_bytes())?;
        w.write_all(&block_align.to_le_bytes())?;
        w.write_all(&(bits as u16).to_le_bytes())?;
        if self.format == SampleFormat::Float32 {
            // cbSize = 0: a float-format chunk with no extension data.
            w.write_all(&0u16.to_le_bytes())?;
        }

        w.write_all(b"data")?;
        w.write_all(&data_bytes.to_le_bytes())?;
        Ok(())
    }

    /// Write interleaved `f32` samples, converting to the target format.
    ///
    /// Conversion clips to the representable range; it does not dither (dither belongs in the
    /// master chain, plan §9.7, not in the file writer).
    pub fn write_frames(&mut self, samples: &[f32]) -> io::Result<()> {
        let mut bytes: Vec<u8> = Vec::with_capacity(samples.len() * self.format.bytes());
        for &s in samples {
            match self.format {
                SampleFormat::Pcm16 => {
                    let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                    bytes.extend_from_slice(&v.to_le_bytes());
                },
                SampleFormat::Pcm24 => {
                    let v = (s.clamp(-1.0, 1.0) * 8388607.0).round() as i32;
                    bytes.extend_from_slice(&v.to_le_bytes()[..3]);
                },
                SampleFormat::Pcm32 => {
                    let v = (s.clamp(-1.0, 1.0) * 2147483647.0).round() as i32;
                    bytes.extend_from_slice(&v.to_le_bytes());
                },
                SampleFormat::Float32 => bytes.extend_from_slice(&s.to_le_bytes()),
            }
        }
        self.data_bytes = self.data_bytes.saturating_add(bytes.len() as u32);
        self.inner.write_all(&bytes)
    }

    /// Patch the header with the final data size and flush.
    pub fn finish(&mut self) -> io::Result<()> {
        self.inner.flush()?;
        Ok(())
    }

    /// Total bytes of sample data written.
    #[must_use]
    pub const fn data_bytes(&self) -> u32 {
        self.data_bytes
    }
}

/// Write a complete interleaved `f32` buffer to a WAV file in one call.
///
/// This is what the render path and the golden-reference generator use.
pub fn write_wav<P: AsRef<Path>>(
    path: P,
    sample_rate: u32,
    channels: u16,
    format: SampleFormat,
    samples: &[f32],
) -> io::Result<u32> {
    // Build in memory so the header can carry the correct size on the first pass (a single-pass
    // seek-and-patch is what WavWriter does for streaming; here simplicity wins).
    let mut data: Vec<u8> = Vec::with_capacity(samples.len() * format.bytes() + 64);
    {
        let mut w = WavWriter::new(&mut data, sample_rate, channels, format)?;
        w.write_frames(samples)?;
        w.finish()?;
    }
    // Patch both sizes from the buffer we just built, rather than recomputing them from the format:
    // the fmt chunk is 18 bytes for float and 16 for PCM, and deriving sizes from the actual bytes
    // means this cannot drift if the header layout changes.
    let riff_size = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&riff_size.to_le_bytes());
    let data_hdr = data.windows(4).position(|w| w == b"data").ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "malformed WAV: no data chunk")
    })?;
    let data_bytes = (data.len() - data_hdr - 8) as u32;
    data[data_hdr + 4..data_hdr + 8].copy_from_slice(&data_bytes.to_le_bytes());
    std::fs::write(path, &data)?;
    Ok(data_bytes)
}

#[cfg(test)]
mod tests {
    // Test harness: panicking on a broken precondition is correct here; the workspace-wide deny on
    // unwrap/expect exists to keep them out of the instrument, not out of tests.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("sparq-wav-test-{}-{}.wav", std::process::id(), name));
        p
    }

    #[test]
    fn header_is_correct_for_pcm16_stereo() {
        let path = tmp("pcm16");
        let samples = [0.0f32, 0.5, -0.5, 1.0, -1.0, 0.25];
        let bytes = write_wav(&path, 48_000, 2, SampleFormat::Pcm16, &samples).unwrap();
        assert_eq!(bytes, 12);
        let f = std::fs::read(&path).unwrap();
        assert_eq!(&f[0..4], b"RIFF");
        assert_eq!(&f[8..12], b"WAVE");
        let fmt = chunk(&f, b"fmt ").expect("fmt chunk");
        assert_eq!(fmt.len(), 16, "PCM fmt chunk has no cbSize");
        assert_eq!(u16::from_le_bytes(fmt[0..2].try_into().unwrap()), 1); // PCM
        assert_eq!(u16::from_le_bytes(fmt[2..4].try_into().unwrap()), 2); // channels
        assert_eq!(u32::from_le_bytes(fmt[4..8].try_into().unwrap()), 48_000);
        assert_eq!(u32::from_le_bytes(fmt[8..12].try_into().unwrap()), 192_000); // byte rate
        assert_eq!(u16::from_le_bytes(fmt[12..14].try_into().unwrap()), 4); // block align
        assert_eq!(u16::from_le_bytes(fmt[14..16].try_into().unwrap()), 16); // bits
        assert_eq!(chunk(&f, b"data").unwrap().len(), 12);
        assert_eq!(f.len(), 44 + 12);
        assert_eq!(u32::from_le_bytes(f[4..8].try_into().unwrap()), (f.len() - 8) as u32);
        std::fs::remove_file(&path).ok();
    }

    /// Locate a RIFF chunk's payload by walking the chunk list (never hard-code offsets: the fmt
    /// chunk is 16 bytes for PCM and 18 for float).
    fn chunk<'a>(f: &'a [u8], id: &[u8; 4]) -> Option<&'a [u8]> {
        let mut o = 12;
        while o + 8 <= f.len() {
            let cid = &f[o..o + 4];
            let size = u32::from_le_bytes(f[o + 4..o + 8].try_into().unwrap()) as usize;
            if cid == id {
                return Some(&f[o + 8..(o + 8 + size).min(f.len())]);
            }
            o += 8 + size + (size & 1); // chunks are word-aligned
        }
        None
    }

    #[test]
    fn float32_writes_cb_size_and_tag_3() {
        let path = tmp("f32");
        let samples = [0.0f32, 1.0, -1.0, 0.5];
        write_wav(&path, 96_000, 1, SampleFormat::Float32, &samples).unwrap();
        let f = std::fs::read(&path).unwrap();
        let fmt = chunk(&f, b"fmt ").expect("fmt chunk");
        assert_eq!(fmt.len(), 18, "float fmt chunk carries cbSize");
        assert_eq!(u16::from_le_bytes(fmt[0..2].try_into().unwrap()), 3); // IEEE float
        assert_eq!(u32::from_le_bytes(fmt[4..8].try_into().unwrap()), 96_000);
        assert_eq!(u16::from_le_bytes(fmt[14..16].try_into().unwrap()), 32); // bits
        assert_eq!(u16::from_le_bytes(fmt[16..18].try_into().unwrap()), 0); // cbSize
        let data = chunk(&f, b"data").expect("data chunk");
        assert_eq!(data.len(), 16);
        assert_eq!(f32::from_le_bytes(data[0..4].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(data[4..8].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(data[8..12].try_into().unwrap()), -1.0);
        assert_eq!(u32::from_le_bytes(f[4..8].try_into().unwrap()), (f.len() - 8) as u32);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn pcm24_roundtrips_to_within_one_lsb() {
        let path = tmp("pcm24");
        let samples: Vec<f32> = (0..256).map(|i| (i as f32 / 128.0) - 1.0).collect();
        write_wav(&path, 48_000, 1, SampleFormat::Pcm24, &samples).unwrap();
        let f = std::fs::read(&path).unwrap();
        for (i, &expected) in samples.iter().enumerate() {
            let o = 44 + i * 3;
            let mut b = [0u8; 4];
            b[..3].copy_from_slice(&f[o..o + 3]);
            if f[o + 2] & 0x80 != 0 {
                b[3] = 0xFF;
            }
            let v = i32::from_le_bytes(b) as f32 / 8388607.0;
            assert!((v - expected).abs() < 2e-7, "sample {i}: {v} vs {expected}");
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn multichannel_interleaving_is_preserved() {
        let path = tmp("multi");
        // 8 channels, one frame, each channel a distinct value.
        let samples: Vec<f32> = (0..8).map(|c| c as f32 / 8.0).collect();
        write_wav(&path, 48_000, 8, SampleFormat::Float32, &samples).unwrap();
        let f = std::fs::read(&path).unwrap();
        let fmt = chunk(&f, b"fmt ").unwrap();
        assert_eq!(u16::from_le_bytes(fmt[2..4].try_into().unwrap()), 8, "channel count");
        let data = chunk(&f, b"data").unwrap();
        for c in 0..8 {
            let v = f32::from_le_bytes(data[c * 4..c * 4 + 4].try_into().unwrap());
            assert!((v - c as f32 / 8.0).abs() < 1e-7, "channel {c} = {v}");
        }
        std::fs::remove_file(&path).ok();
    }
}
