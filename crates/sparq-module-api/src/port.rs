//! The closed port vocabulary of ADR-005, and the connection rules as code.
//!
//! Six port types, and nothing else is ever a port type: new kinds of content are subtypes or
//! `atom` payloads. The set is closed because modules must interoperate across tiers and across
//! years of version drift, and an open type system is how modular ecosystems become a thousand
//! incompatible islands.

use std::fmt;

/// The six port types (ADR-005). Adding a variant requires a new ADR **and** a host major version;
/// removing or renaming one is forbidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PortType {
    /// Float frames, per sample, carrying a [`ChannelSet`].
    Audio,
    /// A single float in a declared range, per sample or per block. The universal modulator.
    Cv,
    /// MIDI 2.0 UMP, OSC, trigger/gate, note (+MPE), clock. Timestamped in `t_sample`.
    Event,
    /// A typed record stream. Timestamped in `t_wall` with a declared rate class.
    Data,
    /// A texture or buffer handle, per frame. Never touched from the audio thread.
    Gpu,
    /// An opaque versioned blob, on demand. Never on the audio thread.
    Atom,
}

impl PortType {
    /// Every port type, in ADR-005's order.
    pub const ALL: [Self; 6] =
        [Self::Audio, Self::Cv, Self::Event, Self::Data, Self::Gpu, Self::Atom];

    /// The manifest spelling of this type.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Cv => "cv",
            Self::Event => "event",
            Self::Data => "data",
            Self::Gpu => "gpu",
            Self::Atom => "atom",
        }
    }

    /// Parses a manifest spelling. `None` means `E-PORT-TYPE-UNKNOWN`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// Whether this type's payload may be touched on the audio thread. `gpu` and `atom` may not.
    #[must_use]
    pub fn audio_thread(self) -> bool {
        !matches!(self, Self::Gpu | Self::Atom)
    }
}

impl fmt::Display for PortType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a channel-set string failed to parse. The two failures have different error codes
/// (`E-CHANNELSET-UNKNOWN` vs `E-AMBI-ORDER-RANGE`), so they are not collapsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelSetError {
    /// Not a layout tag at all.
    UnknownTag,
    /// A well-formed `ambisonics:N` whose order is outside 1..=7 (AmbiX).
    AmbiOrderOutOfRange(u8),
}

/// An audio layout tag (ADR-005 §"ChannelSet layout tags").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelSet {
    /// One channel.
    Mono,
    /// Two channels.
    Stereo,
    /// Four channels.
    Quad,
    /// Six channels, `5.1`.
    FiveOne,
    /// Twelve channels, `7.1.4`.
    SevenOneFour,
    /// AmbiX ambisonics of order 1..=7.
    Ambisonics(u8),
    /// K discrete objects.
    Objects(u16),
    /// M raw channels with no layout meaning; connects only to `raw:M` with equal M.
    Raw(u16),
    /// Resolved at `prepare` to the intersection with the connected neighbour.
    Variable,
}

impl ChannelSet {
    /// The highest AmbiX order sparq declares.
    pub const MAX_AMBI_ORDER: u8 = 7;

    /// Parses a manifest spelling.
    ///
    /// # Errors
    /// [`ChannelSetError::UnknownTag`] for anything that is not a layout tag, or
    /// [`ChannelSetError::AmbiOrderOutOfRange`] for `ambisonics:N` outside 1..=7.
    pub fn parse(s: &str) -> Result<Self, ChannelSetError> {
        match s {
            "mono" => return Ok(Self::Mono),
            "stereo" => return Ok(Self::Stereo),
            "quad" => return Ok(Self::Quad),
            "5.1" => return Ok(Self::FiveOne),
            "7.1.4" => return Ok(Self::SevenOneFour),
            "variable" => return Ok(Self::Variable),
            _ => {},
        }
        if let Some(rest) = s.strip_prefix("ambisonics:") {
            let n: u8 = rest.parse().map_err(|_| ChannelSetError::UnknownTag)?;
            return if (1..=Self::MAX_AMBI_ORDER).contains(&n) {
                Ok(Self::Ambisonics(n))
            } else {
                Err(ChannelSetError::AmbiOrderOutOfRange(n))
            };
        }
        // `objects:K` and `raw:M` carry a count; zero of either is not a layout.
        for (prefix, make) in [("objects:", Self::Objects as fn(u16) -> Self), ("raw:", Self::Raw)]
        {
            if let Some(rest) = s.strip_prefix(prefix) {
                let n: u16 = rest.parse().map_err(|_| ChannelSetError::UnknownTag)?;
                if n == 0 {
                    return Err(ChannelSetError::UnknownTag);
                }
                return Ok(make(n));
            }
        }
        Err(ChannelSetError::UnknownTag)
    }

    /// The channel count, or `None` for [`ChannelSet::Variable`] (unknown until `prepare`).
    #[must_use]
    pub fn channels(self) -> Option<usize> {
        match self {
            Self::Mono => Some(1),
            Self::Stereo => Some(2),
            Self::Quad => Some(4),
            Self::FiveOne => Some(6),
            Self::SevenOneFour => Some(12),
            // AmbiX order N carries (N+1)^2 channels.
            Self::Ambisonics(order) => Some((usize::from(order) + 1).pow(2)),
            Self::Objects(k) => Some(usize::from(k)),
            Self::Raw(m) => Some(usize::from(m)),
            Self::Variable => None,
        }
    }

    /// Whether this set carries spatial meaning, and so refuses to connect to a non-spatial port
    /// without an encoder or decoder in between.
    #[must_use]
    pub fn is_spatial(self) -> bool {
        matches!(self, Self::Ambisonics(_) | Self::Objects(_))
    }
}

impl fmt::Display for ChannelSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mono => f.write_str("mono"),
            Self::Stereo => f.write_str("stereo"),
            Self::Quad => f.write_str("quad"),
            Self::FiveOne => f.write_str("5.1"),
            Self::SevenOneFour => f.write_str("7.1.4"),
            Self::Ambisonics(n) => write!(f, "ambisonics:{n}"),
            Self::Objects(k) => write!(f, "objects:{k}"),
            Self::Raw(m) => write!(f, "raw:{m}"),
            Self::Variable => f.write_str("variable"),
        }
    }
}

/// The rate at which a `cv` port carries values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CvRate {
    /// One value per sample.
    Audio,
    /// One value per block.
    Block,
}

impl CvRate {
    /// Parses a manifest spelling (`audio` | `block`).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "audio" => Some(Self::Audio),
            "block" => Some(Self::Block),
            _ => None,
        }
    }

    /// The manifest spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Block => "block",
        }
    }
}

/// The declared range of a `cv` port.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CvRange {
    /// −1..=1.
    Bipolar,
    /// 0..=1.
    Unipolar,
}

impl CvRange {
    /// Parses a manifest spelling (`bipolar` | `unipolar`).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "bipolar" => Some(Self::Bipolar),
            "unipolar" => Some(Self::Unipolar),
            _ => None,
        }
    }

    /// The manifest spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bipolar => "bipolar",
            Self::Unipolar => "unipolar",
        }
    }

    /// Clamps to the wire's declared closed bounds (−1..=1 bipolar, 0..=1 unipolar). Used by
    /// [`CvInterp::expand`]'s `spline` branch: the parabola can locally exceed its knot span
    /// (all interpolating splines do — the momentum is the smoothness), and the host must never
    /// hand a receiver frames outside the range it declared. `hold` and `linear` cannot exceed
    /// their knots, so they never clamp — an out-of-range knot there is a SOURCE bug and stays
    /// visible rather than being silently rounded away (WO-008 increment 6, decision 4).
    #[must_use]
    pub fn clamp_f64(self, v: f64) -> f64 {
        match self {
            Self::Bipolar => v.clamp(-1.0, 1.0),
            Self::Unipolar => v.clamp(0.0, 1.0),
        }
    }
}

/// How an audio-rate `cv` source collapses into a block-rate input (compat-matrix decision G3:
/// the RECEIVING module declares it, in the manifest, never in code — `first` and `last` render
/// differently, and an undeclared choice would break journal replay, ADR-007).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CvReduce {
    /// The final sample of the block wins. The matrix default.
    #[default]
    Last,
    /// The first sample of the block wins.
    First,
    /// The arithmetic mean over the block.
    Mean,
    /// The minimum over the block.
    Min,
    /// The maximum over the block.
    Max,
    /// The maximum |sample| over the block, sign-preserved at its occurrence — for level
    /// followers, where "the peak" means magnitude, and the sign keeps bipolar cv honest.
    Peak,
}

impl CvReduce {
    /// Parses a manifest spelling (`last` | `first` | `mean` | `min` | `max` | `peak`).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "last" => Some(Self::Last),
            "first" => Some(Self::First),
            "mean" => Some(Self::Mean),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            "peak" => Some(Self::Peak),
            _ => None,
        }
    }

    /// The manifest spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Last => "last",
            Self::First => "first",
            Self::Mean => "mean",
            Self::Min => "min",
            Self::Max => "max",
            Self::Peak => "peak",
        }
    }

    /// Applies the reduction to one block of audio-rate samples. Empty input reduces to 0.0 —
    /// a declared policy over nothing is still nothing, not a guess. Allocation-free.
    #[must_use]
    pub fn apply(self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        match self {
            Self::Last => samples[samples.len() - 1],
            Self::First => samples[0],
            Self::Mean => {
                // f64 accumulator: plan §5.4's rule for every summing path.
                let sum: f64 = samples.iter().map(|s| f64::from(*s)).sum();
                (sum / samples.len() as f64) as f32
            },
            Self::Min => samples.iter().copied().fold(f32::INFINITY, f32::min),
            Self::Max => samples.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            Self::Peak => {
                let mut best = 0.0f32;
                for &s in samples {
                    if s.abs() > best.abs() {
                        best = s;
                    }
                }
                best
            },
        }
    }
}

/// How the host expands a block-rate `cv` source for an audio-rate input (decision G4/Q3: the
/// module declares, the host performs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CvInterp {
    /// Every frame of the block reads the block's value. The matrix default.
    #[default]
    Hold,
    /// A ramp from the PREVIOUS block's value to this block's, reaching this block's value at
    /// frame `frames` (i.e. exactly at the next block boundary): `v[i] = prev + (cur − prev) ·
    /// i / frames`. Continuous across blocks, so a slow block-rate sweep produces no steps.
    Linear,
    /// Performed by the host since WO-008 increment 6: **the parabola through the last three
    /// block values.** With the wire's knot history `prev2` (two blocks ago), `prev` (last block)
    /// and `cur` (this block), frame `i` of `n` reads `t = i/n` on the unique quadratic through
    /// `(−1, prev2), (0, prev), (1, cur)`:
    ///
    /// ```text
    /// v(t) = prev2 · t(t−1)/2  +  prev · (1−t²)  +  cur · t(t+1)/2
    /// ```
    ///
    /// equivalently a cubic Hermite whose start tangent is the central difference
    /// `(cur−prev2)/2` and whose end tangent is the second-order backward estimate
    /// `(3·cur − 4·prev + prev2)/2` — four constraints, one cubic, the same curve. Exact for any
    /// signal quadratic in block index; the exact line for collinear knots (a block-rate ramp
    /// renders bit-identically to `linear`); constant in, constant out. The arrival contract is
    /// `linear`'s: `v(0) = prev` exactly and the curve reaches `cur` at the next block boundary
    /// — declaring `spline` changes the shape of the ride, never its timing. The curve carries
    /// momentum from its history, so it can locally exceed the knot span; the expansion is
    /// clamped to the wire's declared range, which is the whole of its range discipline (see
    /// [`CvInterp::expand`]). Until implemented this spelling was refused at build in words —
    /// the promise the vocabulary made and the host now performs.
    Spline,
}

impl CvInterp {
    /// Parses a manifest spelling (`hold` | `linear` | `spline`).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "hold" => Some(Self::Hold),
            "linear" => Some(Self::Linear),
            "spline" => Some(Self::Spline),
            _ => None,
        }
    }

    /// The manifest spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Linear => "linear",
            Self::Spline => "spline",
        }
    }

    /// Expands the wire's recent block history `prev2 → prev → cur` into `dst` (one frame per
    /// element). Allocation-free; `dst.len()` is the block's frame count. `prev2` is read by
    /// `spline` only — `hold` and `linear` are one knot shorter by definition and their code
    /// paths are untouched by the widening (WO-008 increment 6; the api-snapshot pin moved on
    /// purpose).
    ///
    /// `range` is the wire's declared cv range (the receiver's declaration; G2 proved the source
    /// compatible at build). It is `spline`'s documented range discipline: the parabola through
    /// three knots can locally exceed the knot span — all interpolating splines do; the momentum
    /// IS the smoothness — and the host must never hand a receiver frames outside the range the
    /// receiver declared, so the expansion clamps to `range`. `hold` and `linear` provably never
    /// exceed their knots (a repeat and a convex combination), so they never clamp: an
    /// out-of-range frame there can only come from an out-of-range SOURCE, a module bug that
    /// must stay visible rather than be silently rounded away.
    pub fn expand(self, prev2: f32, prev: f32, cur: f32, range: CvRange, dst: &mut [f32]) {
        let n = dst.len();
        match self {
            Self::Hold => {
                for s in dst.iter_mut() {
                    *s = cur;
                }
            },
            Self::Linear => {
                if n == 0 {
                    return;
                }
                for (i, s) in dst.iter_mut().enumerate() {
                    let t = i as f32 / n as f32;
                    *s = prev + (cur - prev) * t;
                }
            },
            Self::Spline => {
                if n == 0 {
                    return;
                }
                // The parabola through (−1, prev2), (0, prev), (1, cur), evaluated in f64 with
                // one rounding per frame — host-side cv transformations accumulate in f64 (the
                // mixer's cv-sum rule, WO-014 increment 6). Rust never contracts into FMA, so
                // every operation is IEEE-exact per instruction and the render is bit-identical
                // across platforms (ADR-007).
                let (p2, p1, p0) = (f64::from(prev2), f64::from(prev), f64::from(cur));
                for (i, s) in dst.iter_mut().enumerate() {
                    let t = i as f64 / n as f64;
                    let v = p2 * (t * (t - 1.0) / 2.0)
                        + p1 * (1.0 - t * t)
                        + p0 * (t * (t + 1.0) / 2.0);
                    *s = range.clamp_f64(v) as f32;
                }
            },
        }
    }
}

/// Port direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// An input.
    In,
    /// An output.
    Out,
}

impl Direction {
    /// Parses a manifest spelling (`in` | `out`).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "in" => Some(Self::In),
            "out" => Some(Self::Out),
            _ => None,
        }
    }
}

/// How many cables a port accepts. Fan-out is free; fan-in is never implicit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Multiplicity {
    /// Exactly one connection.
    Single,
    /// Many in. For `cv` this requires an explicit merge module; for `event` it is free but the
    /// merge is total-ordered with a stable tie-break (ADR-005 addendum, decision G5).
    MultiIn,
    /// Many out.
    MultiOut,
}

impl Multiplicity {
    /// Parses a manifest spelling (`single` | `multi_in` | `multi_out`).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "single" => Some(Self::Single),
            "multi_in" => Some(Self::MultiIn),
            "multi_out" => Some(Self::MultiOut),
            _ => None,
        }
    }
}

/// A release phase, used to decide whether an offered adapter actually exists yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// The Phase 0 proof of concept.
    Zero,
    /// Kernel and module contract.
    One,
    /// The wasm tier, and with it third-party modules and `dat/mapper`.
    Five,
}

/// A converter module the canvas may offer to insert with one tap.
///
/// ADR-005 addendum: **the canvas may never offer a converter that does not exist.** Each variant
/// therefore carries the phase it first ships in, and [`Verdict::Adapter`] degrades to
/// [`Verdict::Refused`] when asked at an earlier phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Adapter {
    /// `util/range` — bipolar ↔ unipolar `cv` (decision G2). Phase 1.
    Range,
    /// `util/offset` — `cv` → `audio` (ADR-005's "oscillator/offset"). Phase 1.
    Offset,
    /// `ana/rms` — `audio` → `cv` (ADR-005's "analyser"). Already Phase 1 in Appendix B.
    Analyser,
    /// `util/gate-to-cv` — `event` → `cv` (ADR-005's "gate/trigger converter"). Phase 1.
    GateToCv,
    /// `dat/mapper` — `data` → `cv`. Phase 5–7 in Appendix B; until then the edge is refused.
    Mapper,
    /// `util/mixer` — the explicit merge that `cv` fan-in requires. Phase 1.
    Merge,
    /// `spa/hoa-encode` — non-spatial → `ambisonics:N`. Phase 5–7. (Contract v1 split the old
    /// single `Hoa` variant: the matrix names TWO modules, one per direction, and an offer has
    /// to name the module it would insert — defect #80.)
    HoaEncode,
    /// `spa/hoa-decode` — `ambisonics:N` → non-spatial. Phase 5–7.
    HoaDecode,
    /// `spa/objects` — object-count conversion (decision G1). Phase 5–7.
    Objects,
}

impl Adapter {
    /// The module id the canvas would insert.
    #[must_use]
    pub fn module_id(self) -> &'static str {
        match self {
            Self::Range => "util/range",
            Self::Offset => "util/offset",
            Self::Analyser => "ana/rms",
            Self::GateToCv => "util/gate-to-cv",
            Self::Mapper => "dat/mapper",
            Self::Merge => "util/mixer",
            Self::HoaEncode => "spa/hoa-encode",
            Self::HoaDecode => "spa/hoa-decode",
            Self::Objects => "spa/objects",
        }
    }

    /// The first phase in which this adapter exists.
    #[must_use]
    pub fn first_phase(self) -> Phase {
        match self {
            Self::Range | Self::Offset | Self::Analyser | Self::GateToCv | Self::Merge => {
                Phase::One
            },
            Self::Mapper | Self::HoaEncode | Self::HoaDecode | Self::Objects => Phase::Five,
        }
    }

    /// Whether this adapter exists at `phase` — and so may be offered.
    #[must_use]
    pub fn exists_at(self, phase: Phase) -> bool {
        self.first_phase() <= phase
    }
}

/// The outcome of a connection attempt. The canvas renders this directly: compatible ports glow,
/// incompatible dim, and "needs a converter" offers one-tap insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Connects with nothing between it.
    Compatible,
    /// Connects, and the conversion must be drawn — currently only `multi → mono`, which sums and
    /// draws a warning hairline. Nothing is silently up/downmixed except the two documented cases.
    Conversion,
    /// Connects only through a converter module, which exists at `phase`.
    Adapter(Adapter),
    /// Does not connect. Either no converter exists, or one does but not until a later phase.
    Refused,
    /// The matrix has no rule for this cell yet (ADR-005 addendum: only G7, the `gpu` tier
    /// question, deferred to Phase 5).
    Undecided,
}

/// Whether two audio channel sets may connect — the `audio`/`audio` cell of
/// `docs/api/compat-matrix.toml`, case by case.
#[must_use]
pub fn connect_audio(src: ChannelSet, dst: ChannelSet, phase: Phase) -> Verdict {
    use ChannelSet as C;
    match (src, dst) {
        // "src.channel_set == dst.channel_set" -> compatible
        (a, b) if a == b => Verdict::Compatible,
        // `variable` resolves at `prepare` to the intersection with the connected neighbour, and a
        // resolution is a *resource* change, never a graph mutation: the cascade re-prepares
        // dependants in topological order (module-api §2, added 2026-09-22). Nothing here can
        // refuse, because the concrete sets are not known yet — so the verdict is "connects, and
        // the resolution is reported in the UI".
        (C::Variable, _) | (_, C::Variable) => Verdict::Compatible,
        // "mono -> multi fans out" — one of only two documented silent conversions.
        (C::Mono, _) => Verdict::Compatible,
        // "multi -> mono sums and draws a warning hairline".
        (_, C::Mono) => Verdict::Conversion,
        // "raw:M connects only to raw:M with equal M" (equal M is the first arm).
        (C::Raw(_), C::Raw(_)) => Verdict::Refused,
        // objects:K -> objects:J, K != J: decision G1, convert via spa/objects.
        (C::Objects(_), C::Objects(_)) => adapter(Adapter::Objects, phase),
        // "ambisonics:* refuses to connect to non-spatial ports unless an encoder/decoder is
        // inserted" — the table's cell names ambisonics SPECIFICALLY, one module per direction.
        // objects ↔ non-spatial has no named converter, so it falls through to refusal: the
        // canvas never offers a module that does not exist (ADR-005 addendum's rule).
        (C::Ambisonics(_), _) => adapter(Adapter::HoaDecode, phase),
        (_, C::Ambisonics(_)) => adapter(Adapter::HoaEncode, phase),
        // Two different non-spatial fixed layouts (stereo -> quad, 5.1 -> 7.1.4, ...).
        _ => Verdict::Refused,
    }
}

/// Whether two `cv` ranges may connect — decision G2: refused, with `util/range` offered.
#[must_use]
pub fn connect_cv(src: CvRange, dst: CvRange, phase: Phase) -> Verdict {
    if src == dst {
        Verdict::Compatible
    } else {
        adapter(Adapter::Range, phase)
    }
}

/// The cross-type cell: same type required, with the four adapters ADR-005 names.
#[must_use]
pub fn connect_cross(from: PortType, to: PortType, phase: Phase) -> Verdict {
    if from == to {
        return Verdict::Compatible;
    }
    let a = match (from, to) {
        (PortType::Data, PortType::Cv) => Adapter::Mapper,
        (PortType::Cv, PortType::Audio) => Adapter::Offset,
        (PortType::Audio, PortType::Cv) => Adapter::Analyser,
        (PortType::Event, PortType::Cv) => Adapter::GateToCv,
        _ => return Verdict::Refused,
    };
    adapter(a, phase)
}

/// An adapter offer that degrades to a refusal before the adapter's phase — the rule that keeps a
/// button from ever doing nothing.
fn adapter(a: Adapter, phase: Phase) -> Verdict {
    if a.exists_at(phase) {
        Verdict::Adapter(a)
    } else {
        Verdict::Refused
    }
}

#[cfg(test)]
mod tests {
    // Test harness: a failed assertion is the point, and the workspace-wide deny on
    // unwrap/expect/panic exists to keep them out of the instrument, not out of tests.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn port_type_set_is_closed_and_round_trips() {
        assert_eq!(PortType::ALL.len(), 6);
        for t in PortType::ALL {
            assert_eq!(PortType::parse(t.as_str()), Some(t));
        }
        assert_eq!(
            PortType::parse("midi"),
            None,
            "there is no midi port type; it is `event` with a dialect"
        );
    }

    #[test]
    fn gpu_and_atom_are_not_audio_thread_types() {
        for t in PortType::ALL {
            assert_eq!(t.audio_thread(), !matches!(t, PortType::Gpu | PortType::Atom));
        }
    }

    #[test]
    fn channel_sets_round_trip() {
        for s in [
            "mono",
            "stereo",
            "quad",
            "5.1",
            "7.1.4",
            "variable",
            "ambisonics:3",
            "objects:12",
            "raw:8",
        ] {
            let cs = ChannelSet::parse(s).unwrap_or_else(|e| panic!("{s}: {e:?}"));
            assert_eq!(cs.to_string(), s);
        }
    }

    #[test]
    fn ambi_order_out_of_range_is_its_own_error() {
        assert_eq!(ChannelSet::parse("ambisonics:0"), Err(ChannelSetError::AmbiOrderOutOfRange(0)));
        assert_eq!(ChannelSet::parse("ambisonics:8"), Err(ChannelSetError::AmbiOrderOutOfRange(8)));
        // AmbiX order N carries (N+1)^2 channels: 3 -> 16, 7 -> 64.
        assert_eq!(ChannelSet::parse("ambisonics:3").map(|c| c.channels()), Ok(Some(16)));
        assert_eq!(ChannelSet::parse("ambisonics:7").map(|c| c.channels()), Ok(Some(64)));
        assert_eq!(ChannelSet::parse("ambulatory:3"), Err(ChannelSetError::UnknownTag));
        assert_eq!(ChannelSet::parse("objects:0"), Err(ChannelSetError::UnknownTag));
    }

    #[test]
    fn mono_fans_out_and_multi_sums_with_a_warning() {
        let p = Phase::One;
        assert_eq!(connect_audio(ChannelSet::Mono, ChannelSet::Stereo, p), Verdict::Compatible);
        assert_eq!(connect_audio(ChannelSet::Stereo, ChannelSet::Mono, p), Verdict::Conversion);
        assert_eq!(connect_audio(ChannelSet::Stereo, ChannelSet::Stereo, p), Verdict::Compatible);
        assert_eq!(connect_audio(ChannelSet::Stereo, ChannelSet::Quad, p), Verdict::Refused);
    }

    #[test]
    fn raw_connects_only_to_equal_raw() {
        let p = Phase::One;
        assert_eq!(connect_audio(ChannelSet::Raw(8), ChannelSet::Raw(8), p), Verdict::Compatible);
        assert_eq!(connect_audio(ChannelSet::Raw(8), ChannelSet::Raw(4), p), Verdict::Refused);
    }

    #[test]
    fn spatial_refuses_non_spatial_until_an_adapter_exists() {
        let ambi = ChannelSet::Ambisonics(3);
        // Phase 0: spa/hoa-* does not exist yet, so the offer degrades to a refusal rather than
        // presenting a button that does nothing.
        assert_eq!(connect_audio(ambi, ChannelSet::Stereo, Phase::Zero), Verdict::Refused);
        assert_eq!(
            connect_audio(ambi, ChannelSet::Stereo, Phase::Five),
            Verdict::Adapter(Adapter::HoaDecode),
            "leaving spatial decodes"
        );
        assert_eq!(
            connect_audio(ChannelSet::Stereo, ambi, Phase::Five),
            Verdict::Adapter(Adapter::HoaEncode),
            "entering spatial encodes"
        );
        assert_eq!(
            connect_audio(ambi, ChannelSet::Ambisonics(3), Phase::Zero),
            Verdict::Compatible
        );
        // The table's spatial cell names ambisonics specifically; objects → non-spatial has no
        // named converter, so it refuses even where an adapter phase exists (defect #80's fix:
        // the offer must name the module it would insert, and no module renders objects to stereo).
        assert_eq!(
            connect_audio(ChannelSet::Objects(8), ChannelSet::Stereo, Phase::Five),
            Verdict::Refused
        );
    }

    #[test]
    fn object_count_mismatch_needs_a_converter_and_is_never_renumbered() {
        let (a, b) = (ChannelSet::Objects(12), ChannelSet::Objects(8));
        assert_eq!(connect_audio(a, b, Phase::Zero), Verdict::Refused);
        assert_eq!(connect_audio(a, b, Phase::Five), Verdict::Adapter(Adapter::Objects));
        assert_eq!(connect_audio(a, a, Phase::Zero), Verdict::Compatible);
    }

    #[test]
    fn cv_range_mismatch_is_refused_and_offers_util_range() {
        assert_eq!(connect_cv(CvRange::Bipolar, CvRange::Bipolar, Phase::One), Verdict::Compatible);
        assert_eq!(connect_cv(CvRange::Bipolar, CvRange::Unipolar, Phase::Zero), Verdict::Refused);
        assert_eq!(
            connect_cv(CvRange::Bipolar, CvRange::Unipolar, Phase::One),
            Verdict::Adapter(Adapter::Range)
        );
    }

    #[test]
    fn spline_expand_is_the_hand_computed_parabola_and_clamps() {
        // WO-008 increment 6, at the function level (the executor-level proofs live in
        // sparq-audio's tests/cv_spline.rs). Knots (0.5, 1.0, 0.75): the closed form
        // v(t) = 0.5·t(t−1)/2 + 1·(1−t²) + 0.75·t(t+1)/2 simplifies by hand to
        // v(t) = 1 + t/8 − 3t²/8, which exceeds the unipolar bound for every t < 1/3 — so
        // frames i ≤ 21 of 64 clamp to exactly 1.0 and the interior keeps the polynomial
        // (every value dyadic with a small numerator, so the f64→f32 round is exact and `==`
        // is the honest comparison).
        let n = 64;
        let mut dst = vec![f32::NAN; n];
        CvInterp::Spline.expand(0.5, 1.0, 0.75, CvRange::Unipolar, &mut dst);
        for (i, &v) in dst.iter().enumerate() {
            let t = i as f64 / n as f64;
            let want = (1.0 + t / 8.0 - 3.0 * t * t / 8.0).min(1.0) as f32;
            assert_eq!(v, want, "frame {i} of the clamped parabola");
        }
        assert_eq!(dst[0], 1.0, "v(0) is the middle knot, exactly — C⁰ at the knots");
        assert!(
            dst[1..=21].iter().all(|&v| v == 1.0),
            "the overshoot frames clamp to exactly the declared bound"
        );
        assert!(dst[22] < 1.0 && dst[22] > 0.99, "frame 22 is the polynomial again: {}", dst[22]);
        // The mirrored curve on a bipolar wire clamps at the LOWER bound: knots
        // (−0.5, −1.0, −0.75) give v(t) = −(1 + t/8 − 3t²/8), so the same frames clamp to
        // exactly −1.0 — the discipline is the declared range, not a direction.
        let mut wide = vec![f32::NAN; n];
        CvInterp::Spline.expand(-0.5, -1.0, -0.75, CvRange::Bipolar, &mut wide);
        for (i, &v) in wide.iter().enumerate() {
            let t = i as f64 / n as f64;
            let want = (-(1.0 + t / 8.0 - 3.0 * t * t / 8.0)).max(-1.0) as f32;
            assert_eq!(v, want, "frame {i} of the bipolar mirror");
        }
        assert!(wide[1..=21].iter().all(|&v| v == -1.0), "the lower bound clamps exactly");
    }

    #[test]
    fn spline_collapses_to_the_exact_line_and_the_exact_constant() {
        // Collinear knots (0.25, 0.5, 0.75): the unique parabola through three points on a
        // line IS the line, so `spline` must render the same frames `linear` renders for the
        // same outer knots — bit for bit (both sides exact dyadic arithmetic).
        let n = 16;
        let (mut a, mut b) = (vec![0.0f32; n], vec![0.0f32; n]);
        CvInterp::Spline.expand(0.25, 0.5, 0.75, CvRange::Unipolar, &mut a);
        CvInterp::Linear.expand(0.0, 0.5, 0.75, CvRange::Unipolar, &mut b);
        assert_eq!(a, b, "collinear knots: the parabola IS the line, bit for bit");
        // Constant in, constant out — the Lagrange coefficients sum to 1 exactly.
        let mut c = vec![0.0f32; n];
        CvInterp::Spline.expand(0.375, 0.375, 0.375, CvRange::Unipolar, &mut c);
        assert!(c.iter().all(|&v| v == 0.375), "a constant wire is flat at the value: {c:?}");
        // Empty buffers are legal and stay empty (the linear arm's guard, shared).
        CvInterp::Spline.expand(0.0, 0.0, 0.0, CvRange::Unipolar, &mut []);
    }

    #[test]
    fn cross_type_edges_match_the_four_adapters_adr_005_names() {
        let p = Phase::One;
        assert_eq!(
            connect_cross(PortType::Audio, PortType::Cv, p),
            Verdict::Adapter(Adapter::Analyser)
        );
        assert_eq!(
            connect_cross(PortType::Cv, PortType::Audio, p),
            Verdict::Adapter(Adapter::Offset)
        );
        assert_eq!(
            connect_cross(PortType::Event, PortType::Cv, p),
            Verdict::Adapter(Adapter::GateToCv)
        );
        // dat/mapper is Phase 5-7, so at Phase 1 the edge is refused rather than offered.
        assert_eq!(connect_cross(PortType::Data, PortType::Cv, p), Verdict::Refused);
        assert_eq!(
            connect_cross(PortType::Data, PortType::Cv, Phase::Five),
            Verdict::Adapter(Adapter::Mapper)
        );
        // Everything else stays refused: an adapter that is not named does not exist.
        assert_eq!(connect_cross(PortType::Atom, PortType::Audio, p), Verdict::Refused);
        assert_eq!(connect_cross(PortType::Gpu, PortType::Data, p), Verdict::Refused);
        assert_eq!(connect_cross(PortType::Audio, PortType::Audio, p), Verdict::Compatible);
    }

    #[test]
    fn every_adapter_names_a_real_module_id() {
        for a in [
            Adapter::Range,
            Adapter::Offset,
            Adapter::Analyser,
            Adapter::GateToCv,
            Adapter::Mapper,
            Adapter::Merge,
            Adapter::HoaEncode,
            Adapter::HoaDecode,
            Adapter::Objects,
        ] {
            assert!(a.module_id().contains('/'), "{} is not a <category>/<name> id", a.module_id());
        }
        // The exact ids the matrix names — the drift gate (tests/compat_matrix.rs) reads the
        // same strings out of the TOML, so these literals are pinned from both sides.
        assert_eq!(Adapter::HoaEncode.module_id(), "spa/hoa-encode");
        assert_eq!(Adapter::HoaDecode.module_id(), "spa/hoa-decode");
        assert_eq!(Adapter::Objects.module_id(), "spa/objects");
    }
}
