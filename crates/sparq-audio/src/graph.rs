//! The WO-005 signal path: `syn/sine → util/gain → out/main`.
//!
//! This is **not** the module system (WO-007) and **not** the graph executor (WO-008). It is a
//! hard-coded two-node chain whose only jobs are to prove the block contract end to end and to give
//! the golden-reference harness its first subject. It is scheduled for deletion when WO-008 lands,
//! and it is written so that deleting it is a single file removal.
//!
//! What it must demonstrate:
//! * `process` never allocates (asserted by [`Wo005Graph::render`] via the kernel's counting guard),
//! * control changes reach the audio path through a ring and take effect at a block boundary,
//! * the output is bit-reproducible for the same configuration and seed,
//! * a deliberately overloaded callback is counted as an underrun.

use sparq_kernel::block::BlockContext;
use sparq_kernel::device::{NullDevice, RenderConfig, RenderReport, StreamConfig};
use sparq_kernel::rt::Counters;
use std::sync::Arc;

use sparq_kernel::sync::SpscRing;

use crate::dsp::{Gain, SineOsc};

/// A control message from the UI/terminal thread to the audio path.
///
/// `Copy` and `Default` so it can travel through [`SpscRing`] (ADR-005's `atom` port in miniature:
/// a small, serialisable, allocation-free message).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ControlMsg {
    /// Set gain in dB.
    GainDb(f32),
    /// Set oscillator frequency in Hz.
    FreqHz(f64),
    /// Set oscillator amplitude, linear.
    Amp(f32),
    /// No message.
    #[default]
    None,
}

/// Configuration for the WO-005 chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphConfig {
    /// Device/stream configuration.
    pub stream: StreamConfig,
    /// Oscillator frequency in Hz.
    pub freq: f64,
    /// Oscillator amplitude, linear.
    pub amp: f32,
    /// Gain in dB.
    pub gain_db: f32,
    /// Gain smoothing time constant in ms (0 = immediate).
    pub smooth_ms: f32,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            stream: StreamConfig::stereo_48k(),
            freq: 220.0,
            amp: 0.5,
            gain_db: -6.0,
            smooth_ms: 5.0,
        }
    }
}

/// The two-node chain plus its scratch buffers and control ring.
pub struct Wo005Graph {
    cfg: GraphConfig,
    osc: SineOsc,
    gain: Gain,
    /// Mono scratch, `block_frames` long. Allocated in `new`, never in `process`.
    mono: Vec<f32>,
    /// Control messages. The audio path drains this at the top of each block, so a change takes
    /// effect at a block boundary and never mid-block (plan §5.2 rule 2). Behind an `Arc` so the
    /// producer side survives the graph moving into an audio callback (WO-006: the HAL play path
    /// owns the graph on the pump thread while the terminal thread pushes messages — no lock,
    /// per ADR-009 and the clippy.toml note that the audio path takes `&mut` with no lock at all).
    control: Arc<SpscRing<ControlMsg>>,
    /// Blocks processed by this instance.
    blocks: u64,
}

impl Wo005Graph {
    /// Build the chain and allocate every buffer it will ever need.
    #[must_use]
    pub fn new(cfg: GraphConfig) -> Self {
        let mut osc = SineOsc::new(cfg.freq, cfg.amp);
        osc.prepare(cfg.stream.sample_rate);
        let mut gain = Gain::new(cfg.gain_db, cfg.smooth_ms);
        gain.prepare(cfg.stream.sample_rate, cfg.smooth_ms, cfg.stream.outputs.max(1));
        gain.set_db_immediate(cfg.gain_db);
        Self {
            cfg,
            osc,
            gain,
            mono: vec![0.0; cfg.stream.block_frames],
            control: Arc::new(SpscRing::new(64)),
            blocks: 0,
        }
    }

    /// The control ring, for the consumer side (the audio path drains it in `process`).
    #[must_use]
    pub fn control(&self) -> &SpscRing<ControlMsg> {
        &self.control
    }

    /// A producer handle that outlives the graph moving into a callback: clone the `Arc` before
    /// handing the graph to an audio thread, then `push` from the UI/terminal thread. The ring
    /// stays single-producer/single-consumer — the handle does not add a producer, it *is* the
    /// one producer's access path (SPSC discipline is about threads, and only the control thread
    /// ever holds this handle).
    #[must_use]
    pub fn control_handle(&self) -> Arc<SpscRing<ControlMsg>> {
        Arc::clone(&self.control)
    }

    /// The configuration this graph was built with.
    #[must_use]
    pub const fn config(&self) -> &GraphConfig {
        &self.cfg
    }

    /// Process exactly one block into an interleaved output buffer.
    ///
    /// Contract (plan §5.2): no allocation, no locking, no syscall, no wall-clock read, no
    /// unbounded loop. `buf.len()` must equal `ctx.frames * ctx.channels`.
    pub fn process(&mut self, ctx: &BlockContext, buf: &mut [f32]) {
        // 1. Drain control messages. Bounded by the ring capacity, so this cannot loop forever.
        let mut drained = [ControlMsg::None; 8];
        let n = self.control.drain(&mut drained);
        for msg in &drained[..n] {
            match *msg {
                ControlMsg::GainDb(db) => self.gain.set_db(db),
                ControlMsg::FreqHz(f) => self.osc.set_freq(f, ctx.sample_rate),
                ControlMsg::Amp(a) => self.osc.amp = a,
                ControlMsg::None => {},
            }
        }

        // 2. Synthesise mono.
        let frames = ctx.frames.min(self.mono.len());
        self.osc.process(&mut self.mono[..frames]);

        // 3. Fan out to the output channel count (ADR-005's documented mono→multi rule).
        let chans = ctx.channels.max(1);
        for (i, s) in buf.iter_mut().enumerate() {
            *s = self.mono[(i / chans) % frames.max(1)];
        }

        // 4. Gain, with smoothing applied per frame.
        self.gain.process(buf);

        self.blocks += 1;
    }

    /// Render offline, deterministically, with the allocation audit armed.
    #[must_use]
    pub fn render(&mut self, total_frames: usize) -> RenderReport {
        let cfg = RenderConfig::offline(self.cfg.stream, total_frames);
        self.render_with(cfg)
    }

    /// Render with an explicit configuration (e.g. a real-time budget to test underruns).
    pub fn render_with(&mut self, cfg: RenderConfig) -> RenderReport {
        // The closure borrows the DSP nodes mutably but nothing else; the kernel's null device
        // supplies the context and the buffer, exactly as a real backend will.
        let osc = &mut self.osc;
        let gain = &mut self.gain;
        let mono = &mut self.mono;
        let control = &self.control;
        let blocks = &mut self.blocks;
        let mut process = |ctx: &BlockContext, buf: &mut [f32]| {
            let mut drained = [ControlMsg::None; 8];
            let n = control.drain(&mut drained);
            for msg in &drained[..n] {
                match *msg {
                    ControlMsg::GainDb(db) => gain.set_db(db),
                    ControlMsg::FreqHz(f) => osc.set_freq(f, ctx.sample_rate),
                    ControlMsg::Amp(a) => osc.amp = a,
                    ControlMsg::None => {},
                }
            }
            let frames = ctx.frames.min(mono.len());
            osc.process(&mut mono[..frames]);
            let chans = ctx.channels.max(1);
            for (i, s) in buf.iter_mut().enumerate() {
                *s = mono[(i / chans) % frames.max(1)];
            }
            gain.process(buf);
            *blocks += 1;
        };
        NullDevice::render(cfg, &mut process)
    }

    /// Blocks processed so far.
    #[must_use]
    pub const fn blocks_processed(&self) -> u64 {
        self.blocks
    }

    /// Counters for the diagnostics readout.
    #[must_use]
    pub fn counters_from(report: &RenderReport) -> Counters {
        report.counters
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::hash::{fnv1a64_f32, hex64, max_abs_diff};

    #[test]
    fn render_is_bit_reproducible() {
        let cfg = GraphConfig::default();
        let mut a = Wo005Graph::new(cfg);
        let mut b = Wo005Graph::new(cfg);
        let ra = a.render(48_000);
        let rb = b.render(48_000);
        assert_eq!(hex64(fnv1a64_f32(&ra.samples)), hex64(fnv1a64_f32(&rb.samples)));
    }

    #[test]
    fn audio_path_does_not_allocate() {
        let mut g = Wo005Graph::new(GraphConfig::default());
        let rep = g.render(96_000);
        assert_eq!(
            rep.counters.audio_allocations, 0,
            "the audio path allocated {} time(s); plan §5.2 rule 1 forbids it",
            rep.counters.audio_allocations
        );
    }

    #[test]
    fn control_change_takes_effect_and_stays_deterministic() {
        let mut a = Wo005Graph::new(GraphConfig::default());
        let mut b = Wo005Graph::new(GraphConfig::default());
        // Same message sequence, same place in the stream => identical output.
        for g in [&mut a, &mut b] {
            assert!(g.control().push(ControlMsg::GainDb(-18.0)));
        }
        let ra = a.render(24_000);
        let rb = b.render(24_000);
        assert_eq!(fnv1a64_f32(&ra.samples), fnv1a64_f32(&rb.samples));
        // And it must differ from the un-modulated render.
        let mut c = Wo005Graph::new(GraphConfig::default());
        let rc = c.render(24_000);
        assert_ne!(fnv1a64_f32(&ra.samples), fnv1a64_f32(&rc.samples));
    }

    #[test]
    fn underrun_counter_fires_on_a_deliberate_overload() {
        let mut g = Wo005Graph::new(GraphConfig {
            stream: StreamConfig { block_frames: 64, ..StreamConfig::stereo_48k() },
            ..GraphConfig::default()
        });
        // The overload is injected by the *harness* (`RenderConfig::stall`), never by the DSP: a
        // clock read inside a module would break the rule the harness exists to check.
        // 64 frames at 48 kHz = 1.333 ms per block; stall block 2 for 4 ms.
        let mut cfg = RenderConfig::realtime(
            g.config().stream,
            64 * 4,
            RenderConfig::natural_budget(&g.config().stream),
        );
        cfg.stall = Some((2, std::time::Duration::from_millis(4)));
        let rep = g.render_with(cfg);
        assert_eq!(rep.counters.blocks, 4);
        assert!(
            rep.counters.underruns >= 1,
            "the overloaded block must be counted: {:?}",
            rep.counters
        );
    }

    #[test]
    fn quiet_config_renders_near_silence() {
        let mut g = Wo005Graph::new(GraphConfig { gain_db: -120.0, ..GraphConfig::default() });
        let rep = g.render(4096);
        let peak = rep.samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!(peak < 1e-5, "peak {peak} at -120 dB should be ~0");
    }

    #[test]
    fn output_peak_matches_the_configured_gain() {
        let cfg = GraphConfig { amp: 1.0, gain_db: 0.0, smooth_ms: 0.0, ..GraphConfig::default() };
        let mut g = Wo005Graph::new(cfg);
        let rep = g.render(48_000);
        let peak = rep.samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!((peak - 1.0).abs() < 1e-3, "unity gain should peak at 1.0, got {peak}");
    }

    #[test]
    fn multichannel_fanout_is_identical_per_channel() {
        let cfg = GraphConfig {
            stream: StreamConfig { outputs: 8, ..StreamConfig::stereo_48k() },
            ..GraphConfig::default()
        };
        let mut g = Wo005Graph::new(cfg);
        let rep = g.render(512);
        for frame in rep.samples.chunks(8) {
            for s in frame {
                assert!((s - frame[0]).abs() < 1e-7, "mono→8ch fan-out must be identical");
            }
        }
    }

    #[test]
    fn a_one_sample_change_is_reported_with_its_index() {
        let mut a = Wo005Graph::new(GraphConfig::default());
        let ra = a.render(4096);
        let mut rb = ra.samples.clone();
        rb[1234] += 1e-7;
        let (d, i) = max_abs_diff(&ra.samples, &rb);
        assert_eq!(i, 1234);
        assert!(d > 0.0 && d < 1e-6);
        assert_ne!(fnv1a64_f32(&ra.samples), fnv1a64_f32(&rb));
    }
}
