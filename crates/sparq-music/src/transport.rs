//! Transport v0 (WO-009): play/stop, tempo, loop, tap, and the sample-accurate event queue.
//!
//! # The two tick domains, stated once because everything else follows
//!
//! * The **absolute domain** is the kernel map's: monotone, never folded, the truth for
//!   [`Transport::schedule`] — an event scheduled at tick `T` fires at
//!   `broker.sample_at_tick(T)`, which the kernel bisects to ±0 samples (ADR-006 rule 7, and
//!   the acceptance's 1 000-random-tick test).
//! * The **playhead domain** is the transport's: it advances by the map's delta per block and
//!   FOLDS at the loop end — it is what the UI shows and what `position_bar_beat_tick` reads.
//!   Bar/beat triggers deliberately stay on the ABSOLUTE grid (`sample_at_tick`, exact): for a
//!   beat-aligned loop — the sane case — the two grids coincide, and for an unaligned one the
//!   beats keep the global grid rather than inventing a proportional placement that lands one
//!   sample off (measured: the clamped-proportional first draft put boundary beats at 23999
//!   instead of 24000). Loop-relative beat PHASE for unaligned regions is a declared later.
//!
//! Without a loop region the two coincide. With one, the fold happens at BLOCK granularity
//! (declared in the crate header — sub-block transport is §16 Q2's forbidden territory, and the
//! remedy is the same: a smaller host block).
//!
//! # Determinism
//!
//! Everything here is f64 arithmetic over integer sample positions with a fixed operation order
//! — no wall clock enters a decision (the wall side is diagnostics; `tap` takes wall readings as
//! *arguments*, so a replay feeds the same numbers and gets the same tempo). The same script of
//! calls produces the same events at the same samples, which is what "transport stop/start is
//! sample-accurate and repeatable" means mechanically: `stop` FREEZES the timeline (offline tape
//! semantics), so a paused block dispatches nothing and a resumed one continues exactly where
//! the map says.

use sparq_module_api::event::Event;

use crate::broker::ClockBroker;

/// The per-block collector's capacity — equal to the contract's `EVENTS_PER_BLOCK` so one full
/// transport block can feed one event port without dropping on either side.
pub const BLOCK_EVENTS_CAP: usize = 64;

/// The bounded, allocation-free block collector [`Transport::advance_block`] fills and the driver
/// forwards to `Executor::push_host_event`. Overflow is counted (`dropped`/`dropped_total`),
/// never grown — the same discipline as the contract's event sinks.
#[derive(Clone, Copy, Debug)]
pub struct BlockEvents {
    slots: [Event; BLOCK_EVENTS_CAP],
    len: usize,
    dropped: u64,
    dropped_total: u64,
}

impl Default for BlockEvents {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockEvents {
    /// An empty collector.
    #[must_use]
    pub const fn new() -> Self {
        Self { slots: [Event::clock(0); BLOCK_EVENTS_CAP], len: 0, dropped: 0, dropped_total: 0 }
    }

    /// Empties for the next block (per-block drop count resets; the lifetime total does not).
    pub fn clear(&mut self) {
        self.len = 0;
        self.dropped = 0;
    }

    /// Appends one event; `false` means the block's capacity is spent and the event was NOT
    /// stored — counted, never silently dropped.
    pub fn push(&mut self, ev: Event) -> bool {
        if self.len < BLOCK_EVENTS_CAP {
            self.slots[self.len] = ev;
            self.len += 1;
            true
        } else {
            self.dropped += 1;
            self.dropped_total += 1;
            false
        }
    }

    /// The block's events, sorted by sample offset (stable — the sort runs before the driver
    /// sees them, so §9's pre-sorted guarantee holds through the transport door too).
    #[must_use]
    pub fn as_slice(&self) -> &[Event] {
        &self.slots[..self.len]
    }

    /// How many events this block.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the block is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Pushes refused this block.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Pushes refused over the collector's life.
    #[must_use]
    pub const fn dropped_total(&self) -> u64 {
        self.dropped_total
    }

    /// Stable insertion sort by sample (bounded capacity ⇒ bounded work, no allocation — the
    /// `EventBuf::sort_by_sample` reasoning, one level up).
    pub(crate) fn sort_by_sample(&mut self) {
        for i in 1..self.len {
            let mut j = i;
            while j > 0 && self.slots[j].sample < self.slots[j - 1].sample {
                self.slots.swap(j, j - 1);
                j -= 1;
            }
        }
    }
}

/// What the transport emits as graph triggers per block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatConfig {
    /// Emit a trigger on every beat (tick multiples of `ppqn`).
    pub beats: bool,
    /// Emit a trigger on every bar.
    pub bars: bool,
    /// Beats per bar (the v0 time signature — per-track signatures are out of scope).
    pub beats_per_bar: u32,
    /// Channel of beat triggers (a receiving module may filter on it).
    pub beat_channel: u16,
    /// Channel of bar triggers.
    pub bar_channel: u16,
}

impl Default for BeatConfig {
    fn default() -> Self {
        // The drum-demo convention, which the drum-demo golden pins: beats on channel 0, bars on
        // channel 1, four beats per bar, both emitted.
        Self { beats: true, bars: true, beats_per_bar: 4, beat_channel: 0, bar_channel: 1 }
    }
}

/// One tick-scheduled event (absolute domain).
#[derive(Clone, Copy, Debug)]
struct Scheduled {
    tick: u64,
    ev: Event,
}

/// Tap-tempo memory: the last few wall readings, with a 2 s memory — taps across a pause are
/// two tapping sessions, not one tempo.
#[derive(Clone, Copy, Debug, Default)]
struct TapWindow {
    taps: [u64; 4],
    n: usize,
}

impl TapWindow {
    const MEMORY_US: u64 = 2_000_000;

    fn push(&mut self, wall_us: u64) {
        if self.n > 0 && wall_us.saturating_sub(self.taps[self.n - 1]) > Self::MEMORY_US {
            self.n = 0; // the memory expired: this tap starts a new session
        }
        if self.n < self.taps.len() {
            self.taps[self.n] = wall_us;
            self.n += 1;
        } else {
            self.taps.rotate_left(1);
            self.taps[self.taps.len() - 1] = wall_us;
        }
    }

    /// The mean interval over the stored taps, or `None` below two.
    fn mean_interval_us(&self) -> Option<u64> {
        if self.n < 2 {
            return None;
        }
        let span = self.taps[self.n - 1].saturating_sub(self.taps[0]);
        Some(span / (self.n as u64 - 1))
    }
}

/// The transport state machine. One per stream; owns its [`ClockBroker`].
#[derive(Clone, Debug)]
pub struct Transport {
    broker: ClockBroker,
    playing: bool,
    /// Playhead, absolute domain (never folded) — what `schedule` resolves against.
    abs_tick: f64,
    /// Playhead, transport domain (folds at the loop end) — what beats/bars and the UI follow.
    tick_pos: f64,
    loop_region: Option<(f64, f64)>,
    schedule: Vec<Scheduled>,
    cursor: usize,
    beats: BeatConfig,
    next_beat: f64,
    next_bar: f64,
    taps: TapWindow,
    /// The smoothing policy for `set_tempo`: glide time in ms (0 = step). 10 ms is inaudible as
    /// a glide and long enough that tempo-synced DSP does not zipper — a musical default, not a
    /// numerical one.
    ramp_ms: f64,
    /// Per-block guard rails: a beat loop that cannot terminate is worse than a dropped beat.
    emitted_guard: u32,
}

impl Transport {
    /// A transport at `bpm`, stopped, at position zero, with the default beat/bar emission.
    #[must_use]
    pub fn new(sample_rate: u32, bpm: f64, ppqn: u32) -> Self {
        Self {
            broker: ClockBroker::new(sample_rate, bpm, ppqn),
            playing: false,
            abs_tick: 0.0,
            tick_pos: 0.0,
            loop_region: None,
            schedule: Vec::new(),
            cursor: 0,
            beats: BeatConfig::default(),
            next_beat: 0.0,
            next_bar: 0.0,
            taps: TapWindow::default(),
            ramp_ms: 10.0,
            emitted_guard: 0,
        }
    }

    /// A transport with explicit beat/bar emission settings.
    #[must_use]
    pub fn with_beats(sample_rate: u32, bpm: f64, ppqn: u32, beats: BeatConfig) -> Self {
        let mut t = Self::new(sample_rate, bpm, ppqn);
        t.beats = beats;
        t
    }

    /// The broker (the map and the wall side).
    #[must_use]
    pub const fn broker(&self) -> &ClockBroker {
        &self.broker
    }

    /// Mutable broker access — the driver feeds `observe_wall` through here between blocks.
    #[must_use]
    pub fn broker_mut(&mut self) -> &mut ClockBroker {
        &mut self.broker
    }

    /// Whether the transport is playing.
    #[must_use]
    pub const fn is_playing(&self) -> bool {
        self.playing
    }

    /// Starts playback from the current position. Beat/bar phases align to the NEXT tick
    /// multiple at or after the playhead (a play at tick 0 emits beat 0 in the first block).
    pub fn play(&mut self) {
        self.playing = true;
        self.realign_beats();
    }

    /// Stops playback. The timeline FREEZES (offline tape semantics): while stopped,
    /// [`Self::advance_block`] dispatches nothing and the position does not move, so the same
    /// script of play/stop/advance calls always yields the same events at the same samples.
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// The playhead in ticks, transport domain (folded when looping).
    #[must_use]
    pub fn tick_pos(&self) -> f64 {
        self.tick_pos
    }

    /// The playhead in ticks, absolute domain (what `schedule` speaks).
    #[must_use]
    pub fn abs_tick(&self) -> f64 {
        self.abs_tick
    }

    /// The playhead as `bar.beat.tick` (one-based, the kernel's view).
    #[must_use]
    pub fn position_bar_beat_tick(&self) -> (u64, u64, u64) {
        let tick = self.tick_pos.round().max(0.0) as u64;
        self.broker.clock().bar_beat_tick(tick, u64::from(self.beats.beats_per_bar))
    }

    /// The effective tempo now (mid-ramp: between the targets).
    #[must_use]
    pub fn bpm_now(&self) -> f64 {
        self.broker.bpm_now()
    }

    /// Set the tempo, gliding over the default ramp (10 ms). `false` and no change when the
    /// value is not a positive finite tempo — refusals pass through from the map.
    pub fn set_tempo(&mut self, bpm: f64) -> bool {
        self.set_tempo_with_ramp(bpm, self.ramp_ms)
    }

    /// Set the tempo with an explicit glide. `ramp_ms = 0` is a step: position stays continuous
    /// (the map cannot jump), the derivative does not — declared, because "smoothed" is a policy
    /// the caller owns, and a test that wants a step must be able to ask for one.
    pub fn set_tempo_with_ramp(&mut self, bpm: f64, ramp_ms: f64) -> bool {
        self.broker.set_tempo(bpm, ramp_ms)
    }

    /// The loop region in ticks `[start, end)`, transport domain. Refuses (returning `false`,
    /// changing nothing) unless `start < end` and the span is at least one beat — a loop shorter
    /// than a beat would spin the beat emitter, and refusing beats guarding.
    pub fn set_loop(&mut self, region: Option<(u64, u64)>) -> bool {
        if let Some((a, b)) = region {
            if a >= b || (b - a) < u64::from(self.broker.ppqn()) {
                return false;
            }
            self.loop_region = Some((a as f64, b as f64));
            // Fold the PLAYHEAD into the region now (the beat phases ride the absolute grid).
            let (a, b) = (a as f64, b as f64);
            while self.tick_pos >= b {
                self.tick_pos -= b - a;
            }
        } else {
            self.loop_region = None;
        }
        true
    }

    /// The loop region, if any.
    #[must_use]
    pub const fn loop_region(&self) -> Option<(f64, f64)> {
        self.loop_region
    }

    /// Schedule one event at an ABSOLUTE tick (the map's domain). It fires at
    /// `sample_at_tick(tick)` in whichever future block contains that sample — ±0 samples
    /// (ADR-006 rule 7) — including across tempo changes made after scheduling: the MAP is the
    /// truth, so a tempo edit moves the event's sample to where its tick now lives. Control
    /// thread; may allocate (a sorted insert).
    pub fn schedule(&mut self, tick: u64, ev: Event) {
        let at = self.schedule.partition_point(|s| s.tick <= tick);
        self.schedule.insert(at, Scheduled { tick, ev });
        if at < self.cursor {
            // Inserted behind the cursor (a loop of the same tick range, or a locate pending):
            // the cursor rewinds so the walk sees it — expiry logic decides, not insertion order.
            self.cursor = at;
        }
    }

    /// Removes every scheduled event (control thread).
    pub fn clear_schedule(&mut self) {
        self.schedule.clear();
        self.cursor = 0;
    }

    /// How many events are scheduled (diagnostics).
    #[must_use]
    pub fn schedule_len(&self) -> usize {
        self.schedule.len()
    }

    /// Jump the playhead to a tick (both domains). Scheduled events before the new position
    /// expire; after it, they fire as the timeline reaches them. The wall estimator re-anchors
    /// (the broker's rule).
    pub fn locate(&mut self, tick: u64) {
        let sample = self.broker.sample_at_tick(tick);
        self.broker.locate(sample);
        self.abs_tick = tick as f64;
        self.tick_pos = tick as f64;
        self.cursor = 0;
        if let Some((a, b)) = self.loop_region {
            while self.tick_pos >= b {
                self.tick_pos -= b - a;
            }
        }
        self.realign_beats();
    }

    /// One tap-tempo reading (monotonic wall µs — the caller's reading, so a replay is exact).
    /// From the second tap on, sets the tempo from the mean interval over the last ≤4 taps
    /// (2 s memory) and returns the new bpm; the tempo change glides over the default ramp.
    pub fn tap(&mut self, wall_us: u64) -> Option<f64> {
        self.taps.push(wall_us);
        let interval = self.taps.mean_interval_us()?;
        if interval == 0 {
            return None;
        }
        let bpm = 60_000_000.0 / interval as f64;
        let bpm = bpm.clamp(20.0, 300.0);
        self.set_tempo(bpm).then_some(bpm)
    }

    /// Advance one block and collect everything the transport fires inside it, sorted by sample
    /// offset. Allocation-free; bounded by [`BLOCK_EVENTS_CAP`] with counted overflow.
    ///
    /// The driver's contract: call this EXACTLY once per rendered block, forward `out` to the
    /// executor's host-event door, and render. While stopped this is a no-op that returns an
    /// empty collector — the timeline is frozen, not silently ticking.
    pub fn advance_block(&mut self, frames: usize, out: &mut BlockEvents) {
        out.clear();
        if !self.playing || frames == 0 {
            return;
        }
        let s0 = self.broker.sample_pos();
        self.broker.advance(frames);
        let s1 = self.broker.sample_pos();

        // The playhead advances by the MAP's delta across this block — the tempo at any instant
        // inside the block is whatever the segments say, including mid-ramp.
        let delta = self.broker.tick_at_sample_f64(s1) - self.broker.tick_at_sample_f64(s0);
        self.abs_tick += delta;
        self.tick_pos += delta;

        // ---- scheduled events (absolute domain, map-exact samples)
        self.emitted_guard = 0;
        while self.cursor < self.schedule.len() && self.emitted_guard < 4 * BLOCK_EVENTS_CAP as u32
        {
            let e = self.schedule[self.cursor];
            let smp = self.broker.sample_at_tick(e.tick);
            if smp < s0 {
                self.cursor += 1; // expired (a locate or a stop-gap skip): consumed, not fired
                continue;
            }
            if smp >= s1 {
                break; // not yet — the cursor stays, the next block retries
            }
            out.push(Event { sample: (smp - s0) as u32, ..e.ev });
            self.cursor += 1;
            self.emitted_guard += 1;
        }

        // ---- beats and bars (playhead domain; without a loop the two domains coincide and the
        //      samples are map-exact, with a loop the in-block offset is proportional — declared)
        let ppqn = f64::from(self.broker.ppqn());
        let bar_ticks = ppqn * f64::from(self.beats.beats_per_bar.max(1));
        let em = EmitCtx { abs_tick_end: self.abs_tick, s0, s1 };
        // Disjoint-field borrows: the broker (read), the phase (write), the collector (write).
        if self.beats.beats {
            let (ch, period) = (self.beats.beat_channel, ppqn);
            emit_periodic(&self.broker, em, period, &mut self.next_beat, ch, out);
        }
        if self.beats.bars {
            let (ch, period) = (self.beats.bar_channel, bar_ticks);
            emit_periodic(&self.broker, em, period, &mut self.next_bar, ch, out);
        }

        // ---- the loop fold, at block granularity (declared v0 limit). It folds the PLAYHEAD
        // (the display domain) only — the beat/bar phases ride the absolute grid above.
        if let Some((a, b)) = self.loop_region {
            let span = b - a;
            while self.tick_pos >= b {
                self.tick_pos -= span;
            }
        }
        out.sort_by_sample();
    }

    /// Align the beat/bar phases to the ABSOLUTE playhead (play, locate). The phases ride the
    /// map's grid, so a playhead exactly ON a beat fires that beat (the drum-demo's frame-0
    /// kick) and an unaligned playhead waits for the next grid point.
    fn realign_beats(&mut self) {
        let ppqn = f64::from(self.broker.ppqn());
        let bar_ticks = ppqn * f64::from(self.beats.beats_per_bar.max(1));
        self.next_beat = (self.abs_tick / ppqn).ceil() * ppqn;
        self.next_bar = (self.abs_tick / bar_ticks).ceil() * bar_ticks;
    }
}

/// The block a periodic series emits against: the playhead's absolute end tick and the block's
/// half-open sample range.
#[derive(Clone, Copy, Debug)]
struct EmitCtx {
    abs_tick_end: f64,
    s0: u64,
    s1: u64,
}

/// One periodic trigger series (beats or bars): every multiple of `period` ABSOLUTE ticks that
/// the playhead has reached becomes one event, at the map's own `sample_at_tick` — exact, the
/// same inversion the scheduled queue uses, so beats and scheduled events can never disagree
/// about where a tick lives. A free function so the transport's broker read, phase write and
/// collector write stay disjoint borrows — the same discipline the executor's assembly uses.
fn emit_periodic(
    broker: &ClockBroker,
    em: EmitCtx,
    period: f64,
    next: &mut f64,
    channel: u16,
    out: &mut BlockEvents,
) {
    let mut guard = 0u32;
    while *next <= em.abs_tick_end + 1e-9 && guard < 4 * BLOCK_EVENTS_CAP as u32 {
        let sample = broker.sample_at_tick(next.round().max(0.0) as u64);
        if sample >= em.s1 {
            break; // the map puts this beat in a later block — retry there, never twice
        }
        if sample >= em.s0 {
            let mut ev = Event::trigger(0, 1.0);
            ev.sample = (sample - em.s0) as u32;
            ev.channel = channel;
            out.push(ev);
        }
        *next += period;
        guard += 1;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use sparq_module_api::event::EventKind;

    fn t() -> Transport {
        Transport::new(48_000, 120.0, 960)
    }

    #[test]
    fn a_stopped_transport_freezes_the_timeline() {
        let mut tr = t();
        let mut out = BlockEvents::new();
        tr.advance_block(64, &mut out); // stopped: nothing moves
        assert!(out.is_empty());
        assert_eq!(tr.broker().sample_pos(), 0, "the position did not move while stopped");
        tr.play();
        tr.advance_block(64, &mut out);
        assert_eq!(tr.broker().sample_pos(), 64);
        tr.stop();
        tr.advance_block(64, &mut out);
        assert_eq!(tr.broker().sample_pos(), 64, "stop means stop — the tape freezes");
    }

    #[test]
    fn beats_fire_on_the_map_samples_at_120bpm() {
        // 120 bpm @48k: a beat every 24000 samples = every 375 blocks of 64. The drum-demo
        // schedule, derived from the map rather than from a block counter.
        let mut tr = t();
        tr.play();
        let mut out = BlockEvents::new();
        let mut hits: Vec<(u64, u32, u16)> = Vec::new();
        for b in 0..1200u64 {
            tr.advance_block(64, &mut out);
            for e in out.as_slice() {
                if e.kind == EventKind::Trigger && e.channel == 0 {
                    hits.push((b * 64 + u64::from(e.sample), e.sample, e.channel));
                }
            }
        }
        let samples: Vec<u64> = hits.iter().map(|h| h.0).collect();
        // Blocks 0..1200 = 76800 samples = 3.2 beats → beats at 0, 24000, 48000, 72000.
        assert_eq!(samples, vec![0, 24_000, 48_000, 72_000], "the beat grid is the map's");
        assert!(hits.iter().all(|h| h.2 == 0), "beats ride channel 0 (the pinned convention)");
    }

    #[test]
    fn scheduled_events_land_on_the_exact_sample_of_their_tick() {
        let mut tr = t();
        tr.play();
        tr.schedule(960, Event::trigger(0, 0.5)); // one beat in → sample 24000
        tr.schedule(480, Event::trigger(0, 0.25)); // half a beat → sample 12000
        let mut out = BlockEvents::new();
        let mut fired: Vec<(u64, f32)> = Vec::new();
        for b in 0..800u64 {
            tr.advance_block(64, &mut out);
            for e in out.as_slice() {
                if e.value < 0.9 {
                    // the scheduled ones (beats carry value 1.0)
                    fired.push((b * 64 + u64::from(e.sample), e.value));
                }
            }
        }
        assert_eq!(fired, vec![(12_000, 0.25), (24_000, 0.5)], "±0 samples, in map order");
    }

    #[test]
    fn a_tempo_change_after_scheduling_moves_the_event_to_where_its_tick_now_lives() {
        // The MAP is the truth: halving the tempo at sample 0's block boundary doubles every
        // downstream sample position — a scheduled tick fires where the new map puts it.
        let mut tr = t();
        tr.play();
        tr.schedule(960, Event::trigger(0, 0.5));
        let mut out = BlockEvents::new();
        tr.advance_block(64, &mut out); // one block at 120
        assert!(tr.set_tempo_with_ramp(60.0, 0.0)); // step to 60 bpm (test wants exact arithmetic)
        let mut fired_at = None;
        for b in 1..900u64 {
            tr.advance_block(64, &mut out);
            for e in out.as_slice() {
                if e.value < 0.9 {
                    fired_at = Some(b * 64 + u64::from(e.sample));
                }
            }
            if fired_at.is_some() {
                break;
            }
        }
        // From sample 64 (tick 2.56) at 60 bpm (tps 0.02): tick 960 is (960−2.56)/0.02 = 47872
        // samples after 64 → 47936. The kernel's nearest-sample bisection decides the rounding;
        // the claim is the ±1-sample neighbourhood of the analytic value, and ±0 against the map.
        let expect = tr.broker().sample_at_tick(960);
        assert_eq!(fired_at, Some(expect), "the event fires where the map says, post-change");
        assert!(
            (expect as i64 - 47_936).abs() <= 1,
            "and the map says ≈ the analytic sample: {expect}"
        );
    }

    #[test]
    fn the_block_collector_sorts_and_counts() {
        let mut out = BlockEvents::new();
        assert!(out.is_empty());
        assert!(out.push(Event::trigger(30, 1.0)));
        assert!(out.push(Event::trigger(10, 1.0)));
        out.sort_by_sample();
        assert_eq!((out.as_slice()[0].sample, out.as_slice()[1].sample), (10, 30));
        for i in 0..BLOCK_EVENTS_CAP {
            let _ = out.push(Event::trigger(i as u32, 0.0));
        }
        // 62 free slots remained; 2 pushes refused and were counted.
        assert_eq!(out.len(), BLOCK_EVENTS_CAP);
        assert_eq!(out.dropped(), 2, "refusals counted, never grown");
        out.clear();
        assert_eq!(out.dropped(), 0);
        assert_eq!(out.dropped_total(), 2, "the lifetime total survives the block");
    }

    #[test]
    fn tap_tempo_converges_on_the_interval_and_forgets_stale_sessions() {
        let mut tr = t();
        tr.play();
        assert_eq!(tr.tap(0), None, "one tap is not a tempo");
        let got = tr.tap(500_000).unwrap();
        assert!((got - 120.0).abs() < 0.01, "500 ms mean interval = 120 bpm, got {got}");
        tr.tap(1_000_000);
        let got = tr.tap(1_500_000).unwrap();
        assert!((got - 120.0).abs() < 0.01, "steady tapping holds 120: {got}");
        // A 5-second pause expires the memory: the next tap starts a new session (None), and the
        // one after measures from IT, not from before the pause.
        assert_eq!(tr.tap(7_000_000), None, "stale session dropped; one fresh tap is not a tempo");
        let got = tr.tap(7_400_000).unwrap();
        assert!((got - 150.0).abs() < 0.01, "400 ms = 150 bpm, got {got}");
    }

    #[test]
    fn a_loop_folds_the_playhead_and_the_beat_phase() {
        // Two-bar loop [0, 7680) at 120 bpm 4/4 — beat-aligned, so every cycle must look alike.
        let mut tr = t();
        tr.play();
        assert!(tr.set_loop(Some((0, 4 * 2 * 960))));
        assert!(!tr.set_loop(Some((100, 100))), "degenerate region refused");
        assert!(tr.set_loop(Some((0, 4 * 2 * 960))));
        let mut out = BlockEvents::new();
        // One cycle = 7680 TICKS = 8 beats × 24000 samples = 192000 samples = 3000 blocks.
        // (This test's first draft divided ticks by frames and measured quarter-cycles; its
        // second draft found the proportional-placement defect the crate header now records.
        // The beats ride the absolute map grid, so the expectation is the grid itself.)
        const CYCLE_SAMPLES: u64 = 192_000;
        let cycle_blocks = (CYCLE_SAMPLES / 64) as usize;
        let mut cycles: Vec<Vec<u64>> = Vec::new();
        for _c in 0..4u64 {
            let mut offsets = Vec::new();
            for b in 0..cycle_blocks as u64 {
                tr.advance_block(64, &mut out);
                for e in out.as_slice() {
                    if e.channel == 0 {
                        offsets.push(b * 64 + u64::from(e.sample)); // cycle-relative by construction
                    }
                }
            }
            cycles.push(offsets);
        }
        let want: Vec<u64> = (0..8).map(|b| b * 24_000).collect();
        for (i, c) in cycles.iter().enumerate() {
            assert_eq!(c, &want, "cycle {i} is not the beat grid — the fold moved the phase");
        }
        // The playhead itself folded.
        assert!(tr.tick_pos() < 7680.0, "folded playhead: {}", tr.tick_pos());
        assert!(tr.abs_tick() > 20_000.0, "absolute domain never folds: {}", tr.abs_tick());
    }

    #[test]
    fn locate_moves_both_domains_and_reschedules_the_future() {
        let mut tr = t();
        tr.play();
        tr.schedule(4 * 960, Event::trigger(0, 0.5)); // beat 4 → sample 96000 at 120 bpm
        let mut out = BlockEvents::new();
        for _ in 0..100 {
            tr.advance_block(64, &mut out); // 6400 samples in
        }
        tr.locate(2 * 960); // jump to beat 2 (sample 48000)
        assert_eq!(tr.broker().sample_pos(), 48_000);
        assert!((tr.tick_pos() - 1920.0).abs() < 1e-9);
        let mut fired = None;
        for b in 0..900u64 {
            tr.advance_block(64, &mut out);
            for e in out.as_slice() {
                if e.value < 0.9 {
                    fired = Some(48_000 + b * 64 + u64::from(e.sample));
                }
            }
            if fired.is_some() {
                break;
            }
        }
        assert_eq!(fired, Some(96_000), "the scheduled tick still fires at its map sample");
    }
}
