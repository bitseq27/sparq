//! The null/virtual device: a deterministic, hardware-free driver of the block loop.
//!
//! It exists so that (a) CI can run the whole audio path on a runner with no sound card,
//! (b) offline render is bit-reproducible (ADR-007), and (c) the real-time discipline gates —
//! zero allocations, block budget, underrun counting — can be *measured* rather than hoped for.
//!
//! # On timing
//!
//! The null device **does** read the clock (`Instant::now`) once per block, in order to measure
//! block time and to simulate underruns when a budget is set. That is a deliberate property of the
//! *test harness*, not of the audio path: the closure it calls receives only a [`BlockContext`] and
//! buffers, so the code under audit cannot itself observe the wall clock (plan §5.2, ADR-006).
//! Set [`RenderConfig::measure`] to `false` to disable timing entirely and get a pure
//! sample-counted render.

// `Instant::now` is denied workspace-wide (clippy.toml) because the audio path must never read the
// wall clock. This module is the *test harness that measures* the audio path, not the audio path:
// the closure it drives receives only a `BlockContext` and buffers, so the code under audit cannot
// observe the clock. Set `RenderConfig::measure = false` to remove even this.
#![allow(clippy::disallowed_methods)]

use std::time::{Duration, Instant};

use crate::alloc::CountingGuard;
use crate::block::{BlockContext, BlockId};
use crate::device::StreamConfig;
use crate::rt::Counters;

/// How to run a render.
#[derive(Clone, Copy, Debug)]
pub struct RenderConfig {
    /// Device configuration.
    pub stream: StreamConfig,
    /// Total frames to render.
    pub total_frames: usize,
    /// Per-block time budget. `Some(d)` makes the device count an underrun for any block that
    /// exceeds `d` — this is how the underrun counter is proven to work (WO-005 acceptance).
    pub budget: Option<Duration>,
    /// Whether to measure block time at all (see the module docs).
    pub measure: bool,
    /// Whether to audit allocations made inside the block closure.
    pub audit_allocations: bool,
    /// Test hook: busy-wait for this long inside block number `stall_block`, to prove the underrun
    /// counter fires. **The stall lives in the harness, not in the DSP** — a clock read inside a
    /// module would violate the very rule the harness exists to check (clippy denies
    /// `Instant::now` on the audio path for exactly this reason).
    pub stall: Option<(u64, Duration)>,
}

impl RenderConfig {
    /// A deterministic offline render: no timing, no budget.
    #[must_use]
    pub fn offline(stream: StreamConfig, total_frames: usize) -> Self {
        Self {
            stream,
            total_frames,
            budget: None,
            measure: false,
            audit_allocations: true,
            stall: None,
        }
    }

    /// A render that enforces a real-time budget and counts underruns.
    #[must_use]
    pub fn realtime(stream: StreamConfig, total_frames: usize, budget: Duration) -> Self {
        Self {
            stream,
            total_frames,
            budget: Some(budget),
            measure: true,
            audit_allocations: true,
            stall: None,
        }
    }

    /// The block budget implied by the stream config, at 100 % (i.e. exactly real time).
    #[must_use]
    pub fn natural_budget(stream: &StreamConfig) -> Duration {
        Duration::from_secs_f64(stream.block_seconds())
    }
}

/// The result of a render.
#[derive(Clone, Debug)]
pub struct RenderReport {
    /// Interleaved output, `total_frames * outputs` samples.
    pub samples: Vec<f32>,
    /// Counters: blocks, underruns, allocations, drops.
    pub counters: Counters,
    /// Longest observed block processing time (`None` when `measure` was false).
    pub peak_block: Option<Duration>,
    /// Sum of all observed block times (`None` when `measure` was false).
    pub total_block_time: Option<Duration>,
    /// The configuration used.
    pub config: RenderConfig,
}

impl RenderReport {
    /// Mean block time, if measured.
    #[must_use]
    pub fn mean_block(&self) -> Option<Duration> {
        let total = self.total_block_time?;
        if self.counters.blocks == 0 {
            return None;
        }
        Some(total / self.counters.blocks as u32)
    }

    /// Peak block time as a fraction of the configured budget (or the natural budget).
    #[must_use]
    pub fn peak_budget_fraction(&self) -> Option<f64> {
        let peak = self.peak_block?;
        let budget =
            self.config.budget.unwrap_or_else(|| RenderConfig::natural_budget(&self.config.stream));
        if budget.is_zero() {
            return None;
        }
        Some(peak.as_secs_f64() / budget.as_secs_f64())
    }

    /// One-line readout for the diagnostics overlay.
    ///
    /// When timing is disabled (an offline render) the peak field reports the allocation audit
    /// instead of printing `n/a`: there is always *something* worth showing.
    #[must_use]
    pub fn summary(&self) -> String {
        match self.peak_block {
            Some(d) => format!(
                "{} frames · peak block {:.1} µs · budget {:.0}% · {}",
                self.config.total_frames,
                d.as_secs_f64() * 1e6,
                self.peak_budget_fraction().unwrap_or(0.0) * 100.0,
                self.counters.summary()
            ),
            None => format!(
                "{} frames · offline (untimed) · {}",
                self.config.total_frames,
                self.counters.summary()
            ),
        }
    }
}

/// A deterministic virtual audio device.
#[derive(Clone, Copy, Debug)]
pub struct NullDevice;

impl NullDevice {
    /// Drive `process` for `config.total_frames` frames and collect the output.
    ///
    /// `process` receives the block context and an interleaved output slice of exactly
    /// `frames * outputs` samples, which it must fully initialise. The closure cannot allocate
    /// without being counted (when `audit_allocations` is set), cannot see the clock, and cannot
    /// reach the device — that is the whole point.
    pub fn render<P>(config: RenderConfig, mut process: P) -> RenderReport
    where
        P: FnMut(&BlockContext, &mut [f32]),
    {
        let RenderConfig { stream, total_frames, budget, measure, audit_allocations, stall } =
            config;
        if audit_allocations && !crate::alloc::audit_is_live() {
            // Not an error: the audit needs `CountingAllocator` installed as the binary's
            // `#[global_allocator]`, which only test binaries and `sparq-app` do. But it must never
            // *look* like a clean result, so say so.
            eprintln!(
                "sparq-kernel: allocation audit requested but CountingAllocator is not installed \
                 as this binary's #[global_allocator]; audio_allocations will read 0 and means nothing"
            );
        }
        let block = stream.block_frames.max(1);
        let chans = stream.outputs.max(1);
        let mut samples = vec![0.0f32; total_frames * chans];
        let mut scratch = vec![0.0f32; block * chans];
        let mut counters = Counters::zero();
        let mut peak: Option<Duration> = None;
        let mut total = Duration::ZERO;

        let mut ctx = BlockContext {
            block: BlockId::FIRST,
            sample_offset: 0,
            frames: block,
            sample_rate: stream.sample_rate,
            channels: chans,
            tick: 0,
            ppqn: 960,
        };

        let mut written = 0usize;
        while written < total_frames {
            let frames = block.min(total_frames - written);
            ctx.frames = frames;
            let buf = &mut scratch[..frames * chans];

            let guard = audit_allocations.then(CountingGuard::new);
            let started = measure.then(Instant::now);
            process(&ctx, buf);
            if let Some((which, dur)) = stall {
                if ctx.block.0 == which {
                    // Harness-side overload injection: burn wall time so the budget check below
                    // trips, exactly as a real device callback would when a module runs long.
                    let spin = Instant::now();
                    while spin.elapsed() < dur {
                        std::hint::spin_loop();
                    }
                }
            }
            let elapsed = started.map(|t| t.elapsed());
            let allocs = guard.map_or(0, CountingGuard::finish);

            counters.blocks += 1;
            counters.audio_allocations += allocs;
            if let Some(e) = elapsed {
                total += e;
                peak = Some(peak.map_or(e, |p| p.max(e)));
                if let Some(limit) = budget {
                    if e > limit {
                        counters.underruns += 1;
                        // A real device would have consumed the previous buffer again; the null
                        // device models the audible consequence by emitting silence for the block.
                        for s in buf.iter_mut() {
                            *s = 0.0;
                        }
                    }
                }
            }

            let dst = &mut samples[written * chans..(written + frames) * chans];
            dst.copy_from_slice(buf);
            written += frames;

            ctx.block = ctx.block.next();
            ctx.sample_offset += frames as u64;
        }

        RenderReport {
            samples,
            counters,
            peak_block: peak,
            total_block_time: (measure).then_some(total),
            config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_render_produces_exact_frame_count() {
        let cfg = RenderConfig::offline(StreamConfig::stereo_48k(), 1000);
        let rep = NullDevice::render(cfg, |_ctx, buf| {
            for s in buf.iter_mut() {
                *s = 0.5;
            }
        });
        assert_eq!(rep.samples.len(), 2000);
        assert_eq!(rep.counters.blocks, 1000usize.div_ceil(64) as u64);
        assert!(rep.samples.iter().all(|&s| (s - 0.5).abs() < f32::EPSILON));
    }

    #[test]
    fn offline_render_is_deterministic() {
        let cfg = RenderConfig::offline(StreamConfig::stereo_48k(), 4096);
        let run = |seed: u32| {
            NullDevice::render(cfg, move |ctx, buf| {
                for (i, s) in buf.iter_mut().enumerate() {
                    let n = ctx.sample_offset as u32 * 31 + i as u32 * 7 + seed;
                    *s = ((n % 1000) as f32 / 1000.0) - 0.5;
                }
            })
            .samples
        };
        assert_eq!(run(1), run(1), "same inputs must give identical samples (ADR-007)");
        assert_ne!(run(1), run(2));
    }

    #[test]
    fn underrun_counter_fires_when_the_budget_is_exceeded() {
        // An impossible budget: every block overruns. This is the WO-005 acceptance test that the
        // counter is not decorative.
        let cfg = RenderConfig::realtime(StreamConfig::stereo_48k(), 640, Duration::from_nanos(1));
        let rep = NullDevice::render(cfg, |_ctx, buf| {
            let start = Instant::now();
            while start.elapsed() < Duration::from_micros(200) {
                std::hint::spin_loop();
            }
            for s in buf.iter_mut() {
                *s = 0.0;
            }
        });
        assert_eq!(rep.counters.blocks, 10);
        assert_eq!(rep.counters.underruns, 10, "every block blew a 1 ns budget");
        assert!(rep.peak_block.is_some_and(|p| p > Duration::from_nanos(1)));
    }

    #[test]
    fn allocation_audit_needs_the_counting_allocator_installed() {
        // This lib's own test binary uses the system allocator (a `#[global_allocator]` applies to
        // a whole binary, so the kernel cannot install one just for a unit test). The audit is
        // therefore proven where it is actually used: `crates/sparq-audio/tests/rt_discipline.rs`.
        // What THIS test asserts is the more important property: the audit must never *pretend* to
        // have passed.
        let live = crate::alloc::audit_is_live();
        let cfg = RenderConfig::offline(StreamConfig::stereo_48k(), 256);
        let rep = NullDevice::render(cfg, |_ctx, buf| {
            let leaked: Vec<f32> = vec![0.0; 16]; // deliberate violation of plan §5.2 rule 1
            for (i, s) in buf.iter_mut().enumerate() {
                *s = leaked[i % leaked.len()];
            }
        });
        if live {
            assert!(rep.counters.audio_allocations > 0, "a live audit must see the allocation");
        } else {
            assert_eq!(
                rep.counters.audio_allocations, 0,
                "with no counting allocator installed the counter reads 0 -- which is why render() \
                 must warn, and why the real gate lives in rt_discipline.rs"
            );
        }
    }
}
