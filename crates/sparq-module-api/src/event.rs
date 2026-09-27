//! The `event` port payload (ADR-005): timestamped messages, sample-accurate inside the audio
//! clock domain.
//!
//! Contract v1 carries events between modules: an `event` output writes into a bounded
//! [`EventBuf`] through an [`EventSink`]; the executor merges every source of an `event` input
//! into one list **pre-sorted by sample offset** before the receiving module runs (module-api §9's
//! host guarantee), with a stable tie-break — host-injected events first, then connection id, then
//! insertion order (the compat-matrix `event` fan-in cell, decision G5). A receiving module may
//! therefore rely on: `events(i)` is sorted by [`Event::sample`], and equal offsets appear in the
//! order the matrix names.
//!
//! # Real-time discipline
//!
//! Nothing here allocates. A sink is a fixed-capacity window into executor-owned storage
//! ([`EVENTS_PER_BLOCK`] per port per block); a `push` past the capacity returns `false` and is
//! **counted** ([`EventBuf::dropped`]) — bounded, never grown, never silent. The count is the
//! module author's diagnostic: a port that drops every block is lying about its event rate, and
//! the host can say so in words instead of stuttering.
//!
//! # What v1 does NOT decide
//!
//! * Scheduling by musical time (tick → sample) is WO-009's clock broker; v1 events carry a
//!   sample offset inside the current block and nothing else.
//! * Wall-clock streams (OSC, sensors) are quantised per ADR-006 before they become events; that
//!   quantisation is the producer's (host-side) job, never the receiving module's.
//! * An [`Event::sample`] at or beyond the block's frame count is delivered unchanged: the host
//!   guarantees order, not the honesty of a producer's offsets, and a silent clamp would be an
//!   invisible transformation (ADR-005's founding rule). The watchdog and the drop counter are
//!   where misbehaviour surfaces.

use std::fmt;

/// How many events one output port may emit per block. A hard v1 limit in the same family as
/// [`crate::params::MAX_PARAMS`]: the storage is a fixed array so that emitting an event on the
/// audio thread cannot allocate. A module that needs more events per block than this needs a
/// smaller host block (module-api §16 Q2's remedy), not a bigger constant.
pub const EVENTS_PER_BLOCK: usize = 64;

/// The closed vocabulary of event dialects (ADR-005's `event_kinds` domain, also the manifest's
/// `ports[].event_kinds` values). One port type, many dialects: the payload carries which.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// MIDI 2.0 UMP packet (words carry the packet).
    Ump,
    /// OSC message (words carry an encoding of it; the dialect is the payload's business).
    Osc,
    /// A momentary trigger (an onset; `value` is its intensity 0..=1).
    Trigger,
    /// A gate (held while high; `value` is its level).
    Gate,
    /// A note event (+MPE); `channel` is the note channel, `words` the per-note controllers.
    Note,
    /// A clock pulse (transport-derived; WO-009 produces these).
    Clock,
}

impl EventKind {
    /// Every kind, in ADR-005's order.
    pub const ALL: [Self; 6] =
        [Self::Ump, Self::Osc, Self::Trigger, Self::Gate, Self::Note, Self::Clock];

    /// The manifest spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ump => "ump",
            Self::Osc => "osc",
            Self::Trigger => "trigger",
            Self::Gate => "gate",
            Self::Note => "note",
            Self::Clock => "clock",
        }
    }

    /// Parses a manifest spelling. `None` means `E-ENUM-UNKNOWN` at the declaring path.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One event: a dialect, a position inside the block, and a payload. `Copy`, fixed-size, and
/// allocation-free by construction — the whole point of the type.
///
/// Field meanings by kind (the contract's words, not the host's guess):
///
/// | Kind | `value` | `channel` | `words` |
/// |---|---|---|---|
/// | `Trigger` | intensity 0..=1 | stream id (0 default) | unused (zero) |
/// | `Gate` | level (0 = off) | stream id | unused |
/// | `Note` | velocity 0..=1 | note channel | `[key, pressure, 0, 0]`-style per-note controllers |
/// | `Ump` | unused (0.0) | UMP group | the up-to-four packet words |
/// | `Osc` | unused (0.0) | stream id | an encoding of the message (dialect-owned) |
/// | `Clock` | unused (0.0) | 0 | `[ticks_per_pulse as u32, 0, 0, 0]` when transport-derived |
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// Which dialect this event speaks.
    pub kind: EventKind,
    /// Sample offset inside the current block (`0..frames`; the host delivers pre-sorted).
    pub sample: u32,
    /// Channel / stream id, dialect-meaningful (see the table).
    pub channel: u16,
    /// The scalar payload (intensity, level, velocity), dialect-meaningful.
    pub value: f32,
    /// The raw payload words (UMP packets, per-note controllers, OSC encodings).
    pub words: [u32; 4],
}

impl Event {
    /// A trigger at `sample` with the given intensity — the shortest honest constructor.
    #[must_use]
    pub const fn trigger(sample: u32, intensity: f32) -> Self {
        Self { kind: EventKind::Trigger, sample, channel: 0, value: intensity, words: [0; 4] }
    }

    /// A gate at `sample` with the given level.
    #[must_use]
    pub const fn gate(sample: u32, level: f32) -> Self {
        Self { kind: EventKind::Gate, sample, channel: 0, value: level, words: [0; 4] }
    }

    /// A clock pulse at `sample`.
    #[must_use]
    pub const fn clock(sample: u32) -> Self {
        Self { kind: EventKind::Clock, sample, channel: 0, value: 0.0, words: [0; 4] }
    }
}

/// One output port's event storage for one block: a fixed array, a length, and a drop counter.
/// Executor-owned; a module only ever sees it through an [`EventSink`].
///
/// `clear` resets the length but NOT [`Self::dropped_total`] — the totals are the run's evidence,
/// the per-block [`Self::dropped`] resets with the block.
#[derive(Clone, Copy, Debug)]
pub struct EventBuf {
    events: [Event; EVENTS_PER_BLOCK],
    len: usize,
    dropped: u64,
    dropped_total: u64,
}

impl Default for EventBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBuf {
    /// An empty buffer. Stack-constructible and allocation-free, like everything here.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            events: [Event {
                kind: EventKind::Trigger,
                sample: 0,
                channel: 0,
                value: 0.0,
                words: [0; 4],
            }; EVENTS_PER_BLOCK],
            len: 0,
            dropped: 0,
            dropped_total: 0,
        }
    }

    /// Empties the buffer for the next block (the per-block drop count resets; the total does not).
    pub fn clear(&mut self) {
        self.len = 0;
        self.dropped = 0;
    }

    /// The events pushed this block, in insertion order.
    #[must_use]
    pub fn as_slice(&self) -> &[Event] {
        &self.events[..self.len]
    }

    /// How many events this block.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether no events were pushed this block.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The fixed capacity: [`EVENTS_PER_BLOCK`].
    #[must_use]
    pub const fn capacity(&self) -> usize {
        EVENTS_PER_BLOCK
    }

    /// Pushes refused THIS block because the buffer was full — counted, never silently dropped.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Pushes refused over the buffer's whole life.
    #[must_use]
    pub const fn dropped_total(&self) -> u64 {
        self.dropped_total
    }

    /// Sorts the block's events by sample offset, **stably** — equal offsets keep insertion
    /// order, which is the tie-break half of the compat-matrix's G5 cell (the rank half lives in
    /// the merge that combines sources). Insertion sort on purpose: it is stable, allocation-free
    /// and O(n²) against a capacity of [`EVENTS_PER_BLOCK`] — bounded work on the audio thread,
    /// unlike `slice::sort_by`, whose stability costs an allocation the audio path may not make.
    pub fn sort_by_sample(&mut self) {
        for i in 1..self.len {
            let mut j = i;
            while j > 0 && self.events[j].sample < self.events[j - 1].sample {
                self.events.swap(j, j - 1);
                j -= 1;
            }
        }
    }
}

/// What a module's `process` receives for one `event` OUTPUT port: a bounded write window into
/// the executor's [`EventBuf`]. Push returns `false` when the block's capacity is spent — the
/// refusal is counted host-side and reported ([`EventBuf::dropped_total`]), never panicking and
/// never growing on the audio thread.
#[derive(Debug)]
pub struct EventSink<'a> {
    buf: &'a mut EventBuf,
}

impl<'a> EventSink<'a> {
    /// Wraps executor-owned storage. Host-side constructor; a module never builds one.
    #[must_use]
    pub const fn new(buf: &'a mut EventBuf) -> Self {
        Self { buf }
    }

    /// Emits one event. `false` means the port's [`EVENTS_PER_BLOCK`] capacity is spent for this
    /// block; the event was NOT stored and the refusal was counted.
    pub fn push(&mut self, ev: Event) -> bool {
        if self.buf.len < EVENTS_PER_BLOCK {
            self.buf.events[self.buf.len] = ev;
            self.buf.len += 1;
            true
        } else {
            self.buf.dropped += 1;
            self.buf.dropped_total += 1;
            false
        }
    }

    /// Events pushed so far this block.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.buf.len
    }

    /// Whether nothing has been pushed this block.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// The fixed capacity: [`EVENTS_PER_BLOCK`].
    #[must_use]
    pub const fn capacity(&self) -> usize {
        EVENTS_PER_BLOCK
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn kinds_parse_the_closed_domain_and_nothing_else() {
        for k in EventKind::ALL {
            assert_eq!(EventKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(EventKind::parse("midi"), None, "the domain is closed: six dialects");
        assert_eq!(EventKind::parse("TRIGGER"), None, "spellings are exact");
    }

    #[test]
    fn a_sink_fills_in_order_and_refuses_past_capacity_by_counting() {
        let mut buf = EventBuf::new();
        {
            let mut sink = EventSink::new(&mut buf);
            assert!(sink.is_empty());
            for i in 0..EVENTS_PER_BLOCK {
                assert!(sink.push(Event::trigger(i as u32, 0.5)), "push {i} within capacity");
            }
            assert_eq!(sink.len(), EVENTS_PER_BLOCK);
            assert!(!sink.push(Event::trigger(999, 1.0)), "capacity is a wall, not a suggestion");
            assert!(!sink.push(Event::trigger(998, 1.0)));
        }
        assert_eq!(buf.len(), EVENTS_PER_BLOCK);
        assert_eq!(buf.dropped(), 2, "both refusals counted");
        assert_eq!(buf.dropped_total(), 2);
        assert_eq!(buf.as_slice()[0].sample, 0);
        assert_eq!(buf.as_slice()[EVENTS_PER_BLOCK - 1].sample, (EVENTS_PER_BLOCK - 1) as u32);
    }

    #[test]
    fn clear_resets_the_block_but_keeps_the_lifetime_evidence() {
        let mut buf = EventBuf::new();
        {
            let mut sink = EventSink::new(&mut buf);
            for _ in 0..EVENTS_PER_BLOCK + 1 {
                sink.push(Event::clock(0));
            }
        }
        assert_eq!(buf.dropped(), 1);
        buf.clear();
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.dropped(), 0, "per-block count resets with the block");
        assert_eq!(buf.dropped_total(), 1, "the run's evidence survives");
    }

    #[test]
    fn sort_by_sample_is_stable_so_equal_offsets_keep_insertion_order() {
        // G5's tie-break, in the small: the merge ranks SOURCES; within one buffer, insertion
        // order must survive the sort or the guarantee is half a guarantee.
        let mut buf = EventBuf::new();
        {
            let mut sink = EventSink::new(&mut buf);
            // Same samples, distinguishable channels: (5,ch0), (1,ch1), (5,ch2), (1,ch3), (3,ch4).
            for (sample, channel) in [(5u32, 0u16), (1, 1), (5, 2), (1, 3), (3, 4)] {
                sink.push(Event { sample, channel, ..Event::trigger(sample, 0.5) });
            }
        }
        buf.sort_by_sample();
        let got: Vec<(u32, u16)> = buf.as_slice().iter().map(|e| (e.sample, e.channel)).collect();
        assert_eq!(got, vec![(1, 1), (1, 3), (3, 4), (5, 0), (5, 2)], "stable by insertion order");
    }

    #[test]
    fn event_is_copy_and_small_enough_to_pass_around_freely() {
        let e = Event::trigger(12, 0.75);
        let copy = e;
        assert_eq!(e, copy);
        // 1 + 4 + 2 + 4 + 16 bytes of payload, with alignment: a compact fixed record. If this
        // grows past 64 bytes the per-port storage math (EVENTS_PER_BLOCK × ports × nodes) needs
        // a second look, so the size is pinned here rather than discovered in a memory budget.
        assert!(
            std::mem::size_of::<Event>() <= 64,
            "Event grew: {} bytes",
            std::mem::size_of::<Event>()
        );
        assert_eq!(e.kind, EventKind::Trigger);
        assert_eq!(e.sample, 12);
        assert_eq!(e.value, 0.75);
    }

    #[test]
    fn constructors_set_the_dialect_fields_the_table_promises() {
        let g = Event::gate(3, 1.0);
        assert_eq!(g.kind, EventKind::Gate);
        assert_eq!(g.value, 1.0);
        let c = Event::clock(0);
        assert_eq!(c.kind, EventKind::Clock);
        assert_eq!(c.words, [0, 0, 0, 0]);
    }
}
