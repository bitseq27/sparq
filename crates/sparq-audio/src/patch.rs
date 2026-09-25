//! A minimal fixed-topology patch: the Phase B "does it make music?" proof.
//!
//! This is **not** the module system (WO-007) and **not** the graph executor (WO-008). It is a
//! hand-wired chain of the Phase B DSP nodes driven by a step sequencer, and it exists for one
//! reason: to prove the nodes compose into something that sounds like the genre, and to be rendered
//! offline so it can be auditioned on a machine with no audio device (the RDP workflow in
//! `WINDOWS.md`).
//!
//! It is scheduled to be deleted when WO-008 lands, exactly like `graph::Wo005Graph`.
//!
//! Topology:
//!
//! ```text
//!   kick : syn/sine (pitch env) + syn/noise burst -> flt/svf(lp) -> env/ad -> bus
//!   bass : syn/additive saw -> flt/svf(lp, cutoff modulated by env/ad) -> env/adsr -> bus
//!   hat  : syn/noise(brown) -> flt/svf(hp) -> env/ad(fast) -> bus
//!   bus  : util/gain -> [util/delay send] -> out
//! ```
//!
//! Sequencing is sample-accurate via `sparq_kernel::clock::Clock`: step *n* fires at exactly
//! `sample_at_tick(n * ticks_per_step)`, which is asserted in the tests.

use sparq_kernel::clock::Clock;
use sparq_kernel::device::StreamConfig;

use crate::dsp::{
    AdEnv, AdsrEnv, BitCrush, DelayLine, EnvCurve, Gain, Noise, NoiseColour, PolyBlepOsc, SineOsc,
    SvfFilter, SvfMode, WaveShape,
};
use crate::mutation::Op;
use crate::patterns::Pattern;
use crate::presets::{variation, Style};

/// What to render.
// Not `Copy`: the mutation chain is a `Vec<Op>`. Losing `Copy` is correct rather than unfortunate —
// a config that carries a chain of operators is a value that should be passed by reference or
// explicitly cloned, not silently duplicated.
#[derive(Clone, Debug)]
pub struct DemoConfig {
    /// Tempo in beats per minute.
    pub bpm: f64,
    /// Number of bars (4/4, 16 steps per bar).
    pub bars: usize,
    /// Sample rate.
    pub sample_rate: u32,
    /// Output channels (the patch is mono-summed and fanned out).
    pub channels: usize,
    /// Root note for the bass, MIDI number (33 = A1).
    pub root_note: f64,
    /// Bass filter cutoff envelope depth in Hz.
    pub bass_env_amount: f64,
    /// Bass voice level, `0..=1`. Zero mutes the bass, which is how the kick-placement test
    /// isolates the kick — the bass is at 55 Hz and would otherwise dominate the low band.
    pub bass_level: f32,
    /// Length of a bass note as a fraction of one step (`1.0` = legato, `0.6` = plucked).
    pub bass_gate: f32,
    /// Master gain in dB.
    pub gain_db: f32,
    /// Delay send level, `0..=1`.
    pub delay_send: f32,
    /// Bit-crush depth applied to the hat bus (24 = off).
    pub hat_crush_bits: u32,
    /// Deterministic seed for the noise sources **and** for pattern mutation. `0` means "no
    /// mutation": the preset is reproduced exactly, which is what makes a golden reference stable.
    pub seed: u64,
    /// Rhythm style. Replaces the three hardcoded pattern arrays Phase B used.
    pub style: Style,
    /// Mutation chain applied to every percussion lane. Each lane gets its own branch of the seed
    /// tree, so re-seeding the hat never changes the kick (ADR-007).
    pub mutation: Vec<Op>,
    /// Bass detune in semitones, added to `root_note`. Kept separate so `--transpose` can move the
    /// whole patch without touching the style's interval choices.
    pub transpose: i8,
    /// Solo one voice. Added to diagnose a silent kick and kept because it is useful for mixing.
    pub solo: Solo,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            bpm: 138.0,
            bars: 4,
            sample_rate: 48_000,
            channels: 2,
            root_note: 33.0,
            bass_env_amount: 2_600.0,
            bass_level: 0.55,
            bass_gate: 0.7,
            gain_db: -6.0,
            delay_send: 0.25,
            hat_crush_bits: 24,
            seed: 0xA17E,
            solo: Solo::None,
            style: Style::Techno,
            mutation: Vec::new(),
            transpose: 0,
        }
    }
}

/// Solo a single voice, for diagnosis and for mixing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Solo {
    /// Nothing soloed: the full mix.
    #[default]
    None,
    /// Kick only.
    Kick,
    /// Bass only.
    Bass,
    /// Hat only.
    Hat,
}

/// Steps per bar at sixteenth resolution. The style presets are all 16 steps for the kick lane, but
/// lanes may be *any* length — that is the polymeter, and `PatternSet::cycle_steps()` reports when
/// the whole set repeats.
const STEPS_PER_BAR: usize = 16;

struct Voices {
    // kick
    kick_sine: SineOsc,
    kick_pitch_env: AdEnv,
    kick_noise: Noise,
    kick_noise_env: AdEnv,
    kick_amp: AdEnv,
    kick_lp: SvfFilter,
    // bass
    bass_osc: PolyBlepOsc,
    bass_amp: AdsrEnv,
    bass_filt: SvfFilter,
    bass_filt_env: AdEnv,
    // hat
    hat_noise: Noise,
    hat_amp: AdEnv,
    hat_hp: SvfFilter,
    hat_crush: BitCrush,
    // bus
    delay: DelayLine,
    master: Gain,
}

impl Voices {
    fn new(cfg: &DemoConfig) -> Self {
        let sr = cfg.sample_rate;

        let mut kick_sine = SineOsc::new(50.0, 1.0);
        kick_sine.prepare(sr);

        // Pitch envelope: 10 ms attack to the peak, 55 ms decay — the click-then-body of a 909.
        let mut kick_pitch_env = AdEnv::new(0.5, 55.0);
        kick_pitch_env.curve = EnvCurve::Exp;
        kick_pitch_env.prepare(sr);

        let mut kick_noise = Noise::new(cfg.seed);
        kick_noise.colour = NoiseColour::White;
        kick_noise.amp = 0.6;

        let mut kick_noise_env = AdEnv::new(0.2, 18.0);
        kick_noise_env.prepare(sr);

        let mut kick_amp = AdEnv::new(0.4, 260.0);
        kick_amp.prepare(sr);

        let mut kick_lp = SvfFilter::new(320.0, 0.25);
        kick_lp.prepare(sr);

        let mut bass_osc = PolyBlepOsc::new(crate::dsp::note_to_hz(cfg.root_note), 0.5);
        bass_osc.set_shape(WaveShape::Saw);
        // A1 (55 Hz) at 48 kHz would build ~436 partials and call sin() that many times per sample,
        // which made a two-bar render take seconds. 48 partials reaches 2.6 kHz — plenty for a
        // filtered bass line — at an eighth of the cost. This is the additive-synthesis CPU trade
        // documented in dsp/osc.rs, made visible at the place it is paid.
        bass_osc.set_max_partials(48);
        bass_osc.prepare(sr);

        let mut bass_amp = AdsrEnv::new(4.0, 180.0, 0.55, 120.0);
        bass_amp.prepare(sr);

        let mut bass_filt = SvfFilter::new(320.0, 0.62);
        bass_filt.mode = SvfMode::LowPass;
        bass_filt.prepare(sr);

        let mut bass_filt_env = AdEnv::new(1.0, 240.0);
        bass_filt_env.prepare(sr);

        let mut hat_noise = Noise::new(cfg.seed.wrapping_mul(31).wrapping_add(7));
        hat_noise.colour = NoiseColour::Brown;
        hat_noise.amp = 0.9;

        let mut hat_amp = AdEnv::new(0.3, 45.0);
        hat_amp.prepare(sr);

        let mut hat_hp = SvfFilter::new(7_200.0, 0.3);
        hat_hp.mode = SvfMode::HighPass;
        hat_hp.prepare(sr);

        let mut hat_crush = BitCrush::new(cfg.hat_crush_bits, 1);
        hat_crush.prepare(sr);

        let mut delay = DelayLine::new(1.5);
        delay.feedback = 0.34;
        delay.damp = 0.55;
        delay.mix = 1.0; // used as a send, so the return is fully wet
        delay.prepare(sr);
        // Dotted-eighth send: the classic techno/IDM delay time.
        delay.set_tempo_sync(cfg.bpm, 1.0, 1.5);

        let mut master = Gain::new(cfg.gain_db, 2.0);
        master.prepare(sr, 2.0, cfg.channels.max(1));

        Self {
            kick_sine,
            kick_pitch_env,
            kick_noise,
            kick_noise_env,
            kick_amp,
            kick_lp,
            bass_osc,
            bass_amp,
            bass_filt,
            bass_filt_env,
            hat_noise,
            hat_amp,
            hat_hp,
            hat_crush,
            delay,
            master,
        }
    }
}

/// Render the demo patch to interleaved `f32`.
///
/// Allocation happens once, up front; the sample loop allocates nothing, which the
/// `render_is_allocation_free` test asserts.
pub fn render_demo(cfg: &DemoConfig) -> Vec<f32> {
    let sr = cfg.sample_rate;
    let chans = cfg.channels.max(1);
    let ppqn = 960u32;
    let clock = Clock::new(sr, cfg.bpm, ppqn);
    // 16 steps per bar in 4/4 => a step is a sixteenth => ppqn/4 ticks.
    let ticks_per_step = u64::from(ppqn) / 4;
    let total_steps = cfg.bars * STEPS_PER_BAR;
    let total_frames = clock.sample_at_tick(u64::from(ppqn) * 4 * cfg.bars as u64) as usize;

    let mut out = vec![0.0f32; total_frames * chans];
    let mut v = Voices::new(cfg);

    // ---- patterns (Phase C1) -------------------------------------------------------
    // Phase B had three `const [bool; 16]` arrays in this file. Those are gone: a rhythm is now a
    // value that can be generated, mutated, seeded and recorded in a lineage.
    let (perc, _mutation_log) = variation(cfg.style, ppqn, &cfg.mutation, cfg.seed);
    let kick_pat = perc.get("kick").cloned().unwrap_or_else(|| Pattern::euclidean(4, 16, 0, ppqn));
    let hat_pat = perc.get("hat").cloned().unwrap_or_else(|| Pattern::euclidean(2, 4, 2, ppqn));
    let bass_semitones = cfg.style.bass_semitones();
    let bass_steps = bass_semitones.len().max(1) as u64;

    // Precompute the sample index at which each global step fires, *without* microtiming. Per-step
    // offsets are applied when the step is consumed, because a negative offset must never be allowed
    // to schedule a hit in the past — clamping at fire time keeps that correct by construction.
    let mut step_at = Vec::with_capacity(total_steps + 1);
    for s in 0..=total_steps {
        step_at.push(clock.sample_at_tick(s as u64 * ticks_per_step) as usize);
    }
    let frames_per_step = {
        let f = clock.samples_per_beat() / 4.0;
        if f < 1.0 {
            1.0
        } else {
            f
        }
    };

    let bass_env_amount = cfg.bass_env_amount;
    let delay_send = cfg.delay_send.clamp(0.0, 1.0);

    let mut next_step = 0usize;
    let mut bass_note = cfg.root_note;
    // Sample at which the currently-sounding bass note is released, if any.
    let mut bass_off_at: Option<usize> = None;
    let gate_len = ((step_at[1].saturating_sub(step_at[0])) as f64
        * f64::from(cfg.bass_gate.clamp(0.05, 1.0))) as usize;
    // Per-lane RNGs for probability rolls, branched off the same seed tree as the variation. Each
    // lane therefore has its own reproducible stream: changing the hat's probability cannot alter
    // the kick's rolls (ADR-007).
    let lane_tree = sparq_kernel::seed::SeedTree::new(cfg.seed);
    let mut kick_rng = lane_tree.child("roll").child("kick").rng();
    let mut hat_rng = lane_tree.child("roll").child("hat").rng();
    let mut kick_vel = 1.0f32;
    let mut hat_vel = 1.0f32;
    let mut pending_ratchet: usize = 0;
    let mut ratchet_every: f64 = 0.0;

    for frame in 0..total_frames {
        // ---- sequencer: fire triggers exactly on their sample ----
        while next_step < total_steps {
            let due = step_at[next_step];
            // Microtiming: the kick lane's own offset moves its hit. Clamped so it can never push a
            // hit before the previous step, which would make the sequencer fire out of order.
            let micro = kick_pat
                .steps()
                .get(next_step % kick_pat.len().max(1))
                .map_or(0, |st| st.offset_ticks);
            let offset_samples = if micro == 0 {
                0i64
            } else {
                (f64::from(micro) / f64::from(ppqn) * frames_per_step).round() as i64
            };
            let fire_at = (due as i64 + offset_samples).max(0) as usize;
            if fire_at > frame {
                break;
            }

            let ki = next_step % kick_pat.len().max(1);
            if let Some(st) = kick_pat.steps().get(ki) {
                if st.is_on() && st.fires(&mut kick_rng) {
                    v.kick_pitch_env.trigger();
                    v.kick_noise_env.trigger();
                    v.kick_amp.trigger();
                    kick_vel = st.velocity;
                }
            }

            let hi = next_step % hat_pat.len().max(1);
            if let Some(st) = hat_pat.steps().get(hi) {
                if st.is_on() && st.fires(&mut hat_rng) {
                    v.hat_amp.trigger();
                    hat_vel = st.velocity;
                    // A ratchet subdivides this step into n equal repeats: the breakcore roll. The
                    // extra hits are fired by the sub-step check below, not here, so they stay
                    // sample-accurate rather than bunched at the step boundary.
                    pending_ratchet = usize::from(st.ratchet.max(1)) - 1;
                    ratchet_every = if pending_ratchet > 0 {
                        frames_per_step / (pending_ratchet + 1) as f64
                    } else {
                        0.0
                    };
                } else {
                    pending_ratchet = 0;
                }
            }

            if let Some(Some(semi)) = bass_semitones.get(next_step % bass_steps as usize) {
                bass_note = cfg.root_note + f64::from(*semi) + f64::from(cfg.transpose);
                v.bass_amp.gate(true);
                v.bass_filt_env.trigger();
                bass_off_at = Some(frame + gate_len);
            }
            next_step += 1;
        }

        // ---- ratchet sub-steps (rolls) ----
        if pending_ratchet > 0 && ratchet_every >= 1.0 {
            let pos_in_step = (frame as f64) % frames_per_step;
            let idx = (pos_in_step / ratchet_every) as usize;
            // Fire once per sub-slot, guarded by the remaining count so a step can never fire twice.
            if idx >= 1
                && idx <= pending_ratchet
                && (frame as f64 - pos_in_step + idx as f64 * ratchet_every) as usize == frame
            {
                v.hat_amp.trigger();
                hat_vel *= 0.8;
                pending_ratchet -= 1;
            }
        }
        // Note-off, scheduled from the step grid rather than from an ad-hoc bar boundary. The first
        // version released only on `s % 4 == 0` when there was no note, which held notes for up to
        // a whole bar and turned the bass into a drone that masked the kick.
        if bass_off_at.is_some_and(|at| frame >= at) {
            v.bass_amp.gate(false);
            bass_off_at = None;
        }
        if next_step >= total_steps && bass_off_at.is_none() {
            v.bass_amp.gate(false);
        }
        // ---- kick: sine with a pitch envelope, plus a short noise transient ----
        let pitch_env = v.kick_pitch_env.tick();
        // 50 Hz body, up to ~180 Hz at the transient: the "click" that makes a kick cut through.
        let kick_freq = 50.0 + 130.0 * f64::from(pitch_env);
        v.kick_sine.set_freq(kick_freq, sr);
        let kick_body = v.kick_sine.process_mono();
        let kick_click = v.kick_noise.tick() * v.kick_noise_env.tick();
        // `tick()` both advances the envelope and returns its level. Reading `level()` without
        // ticking leaves the state machine frozen in Attack at zero forever — which is exactly the
        // bug this line originally had, and why the kick measured exactly 0.0 while the hat (which
        // did tick) worked. Envelopes must be driven, not sampled.
        let mut kick = (kick_body * 0.9 + kick_click * 0.5) * v.kick_amp.tick();
        kick = v.kick_lp.tick(kick);

        // ---- bass: saw -> env-modulated lowpass -> amp envelope ----
        v.bass_osc.set_freq(crate::dsp::note_to_hz(bass_note));
        let bass_raw = v.bass_osc.tick_free();
        let fenv = v.bass_filt_env.tick();
        v.bass_filt.set_cutoff(240.0 + bass_env_amount * f64::from(fenv));
        let bass = v.bass_filt.tick(bass_raw) * v.bass_amp.tick() * cfg.bass_level.clamp(0.0, 1.0);

        // ---- hat: brown noise -> highpass -> optional crush -> fast amp envelope ----
        let hat_raw = v.hat_noise.tick();
        let hat_filtered = v.hat_hp.tick(hat_raw);
        let hat_crushed = v.hat_crush.tick(hat_filtered);
        let hat = hat_crushed * v.hat_amp.tick();

        // ---- bus ----
        let (kl, bl, hl) = match cfg.solo {
            Solo::None => (0.85, 0.55, 0.30),
            Solo::Kick => (1.0, 0.0, 0.0),
            Solo::Bass => (0.0, 1.0, 0.0),
            Solo::Hat => (0.0, 0.0, 1.0),
        };
        let mut mix = kick * kl * kick_vel + bass * bl + hat * hl * hat_vel;
        let wet = v.delay.tick(mix * delay_send);
        mix += wet * 0.55;
        mix = mix.clamp(-1.2, 1.2);

        for c in 0..chans {
            out[frame * chans + c] = mix;
        }
    }

    // Master gain with smoothing, applied over the interleaved buffer.
    v.master.process(&mut out);
    out
}

/// The stream config this demo renders at, for the WAV writer.
#[must_use]
pub fn demo_stream(cfg: &DemoConfig) -> StreamConfig {
    StreamConfig {
        sample_rate: cfg.sample_rate,
        block_frames: 64,
        inputs: 0,
        outputs: cfg.channels.max(1),
        exclusive: false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::hash::fnv1a64_f32;

    /// A deliberately cheap config for the musical assertions: half a bar at 24 kHz is plenty to
    /// check tuning, determinism, gating and level, and keeps the suite in seconds rather than
    /// minutes. Fidelity-sensitive tests use 48 kHz explicitly.
    fn small() -> DemoConfig {
        DemoConfig {
            bars: 1,
            sample_rate: 24_000,
            channels: 2,
            bpm: 180.0,
            ..DemoConfig::default()
        }
    }

    #[test]
    fn render_produces_the_expected_frame_count() {
        let cfg = small();
        let out = render_demo(&cfg);
        // One bar of 4/4 at 120... at cfg.bpm: 4 beats.
        let clock = Clock::new(cfg.sample_rate, cfg.bpm, 960);
        let expected = clock.sample_at_tick(960 * 4) as usize * cfg.channels;
        assert_eq!(out.len(), expected, "frame count should match four beats at {} bpm", cfg.bpm);
    }

    #[test]
    fn render_is_deterministic() {
        let cfg = small();
        let a = render_demo(&cfg);
        let b = render_demo(&cfg);
        assert_eq!(
            fnv1a64_f32(&a),
            fnv1a64_f32(&b),
            "ADR-007: same config and seed must be bit-identical"
        );
    }

    #[test]
    fn different_seeds_give_different_audio() {
        let a = render_demo(&DemoConfig { seed: 1, ..small() });
        let b = render_demo(&DemoConfig { seed: 2, ..small() });
        assert_ne!(fnv1a64_f32(&a), fnv1a64_f32(&b), "the seed must reach the noise sources");
    }

    #[test]
    fn tempo_changes_the_length() {
        let slow = render_demo(&DemoConfig { bpm: 90.0, ..small() });
        let fast = render_demo(&DemoConfig { bpm: 270.0, ..small() });
        assert!(
            slow.len() as f64 > fast.len() as f64 * 2.5,
            "90 bpm should be ~3x the length of 270 bpm"
        );
    }

    #[test]
    fn output_is_bounded_and_not_silent() {
        let cfg = small();
        let out = render_demo(&cfg);
        let peak = out.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!(out.iter().all(|s| s.is_finite()), "non-finite samples in the render");
        assert!(peak > 0.05, "render is essentially silent: peak {peak}");
        assert!(peak <= 1.2, "render exceeds the internal headroom limit: peak {peak}");
    }

    #[test]
    fn the_kick_lands_on_the_quarter_notes() {
        // Measure the LOW band, not full bandwidth: the kick is a 50 Hz body with a short
        // transient, and at full bandwidth the hats (which fire on the off-beat eighth) compete with
        // it. The first version of this test used broadband RMS and failed on a correctly-placed
        // kick for exactly that reason. The one-pole below has a cutoff around 97 Hz at 24 kHz.
        let cfg = DemoConfig {
            bars: 2,
            sample_rate: 24_000,
            channels: 1,
            bpm: 180.0,
            delay_send: 0.0,
            bass_level: 0.0, // isolate the kick: the bass is at 55 Hz and would dominate this band
            ..DemoConfig::default()
        };
        let raw = render_demo(&cfg);
        let mut lp = Vec::with_capacity(raw.len());
        let mut s = 0.0f32;
        for &x in &raw {
            s += 0.05 * (x - s);
            lp.push(s);
        }
        let clock = Clock::new(cfg.sample_rate, cfg.bpm, 960);
        let beat = clock.sample_at_tick(960) as usize;
        let span = beat / 4;
        let rms = |centre: usize| {
            let lo = centre.saturating_sub(span / 2);
            let hi = (centre + span / 2).min(lp.len());
            let sl = &lp[lo..hi];
            (sl.iter().map(|&x| f64::from(x) * f64::from(x)).sum::<f64>() / sl.len().max(1) as f64)
                .sqrt()
        };
        for b in 0..7 {
            let on = rms(beat * b + span / 2);
            let off = rms(beat * b + beat / 2 + span / 2);
            assert!(
                on > off * 2.0,
                "beat {b}: low-band on-beat rms {on:.5} should clearly exceed off-beat {off:.5}"
            );
        }
    }

    #[test]
    fn bass_gate_length_changes_the_note_duration() {
        // A plucked gate must leave audible silence between notes; a legato gate must not.
        let energy = |gate: f32| {
            // 90 bpm, not the 180 bpm the other tests use: a sixteenth at 180 bpm is 83 ms, which
            // is shorter than the bass amp release (120 ms), so every gate setting collapses to
            // "held" and the test measures nothing. At 90 bpm a step is 166 ms and the difference
            // between a 0.2 and a 1.0 gate is audible in the energy.
            let cfg = DemoConfig {
                bars: 1,
                sample_rate: 24_000,
                channels: 1,
                bpm: 90.0,
                delay_send: 0.0,
                bass_gate: gate,
                // Solo the bass, or the kick dominates total energy and the gate difference
                // disappears into it (that is why the first version of this test failed even though
                // the gate mechanism works — measured soloed, energy goes 81 -> 130 -> 196).
                solo: Solo::Bass,
                // Zero the filter sweep too, or the test measures two envelopes at once: the filter
                // envelope decays over 240 ms, so a LONGER gate means a DARKER note.
                bass_env_amount: 0.0,
                ..DemoConfig::default()
            };
            let out = render_demo(&cfg);
            out.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
        };
        let plucked = energy(0.2);
        let legato = energy(1.0);
        assert!(
            legato > plucked * 1.15,
            "legato gate should carry more energy: {legato} vs {plucked}"
        );
    }

    #[test]
    fn every_style_renders_and_is_audible() {
        for style in Style::all() {
            let cfg = DemoConfig {
                bars: 2,
                sample_rate: 24_000,
                channels: 1,
                bpm: 150.0,
                delay_send: 0.0,
                style,
                ..DemoConfig::default()
            };
            let out = render_demo(&cfg);
            let peak = out.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
            let rms = (out.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
                / out.len() as f64)
                .sqrt();
            assert!(peak > 0.05, "{style:?} is near-silent: peak {peak}");
            assert!(peak <= 1.2, "{style:?} exceeds headroom: peak {peak}");
            assert!(rms > 0.005, "{style:?} has no energy: rms {rms}");
            assert!(out.iter().all(|s| s.is_finite()), "{style:?} produced non-finite samples");
        }
    }

    #[test]
    fn styles_sound_different_from_each_other() {
        let render = |style| {
            let cfg = DemoConfig {
                bars: 2,
                sample_rate: 24_000,
                channels: 1,
                bpm: 150.0,
                delay_send: 0.0,
                style,
                seed: 0,
                ..DemoConfig::default()
            };
            render_demo(&cfg)
        };
        let techno = render(Style::Techno);
        for other in [Style::Breakcore, Style::Glitch, Style::Driving] {
            let x = render(other);
            assert_ne!(
                fnv1a64_f32(&techno),
                fnv1a64_f32(&x),
                "{other:?} rendered identically to techno"
            );
        }
    }

    #[test]
    fn seed_zero_reproduces_the_preset_and_nonzero_seeds_vary_it() {
        let base = DemoConfig {
            bars: 1,
            sample_rate: 24_000,
            channels: 1,
            bpm: 150.0,
            delay_send: 0.0,
            style: Style::Breakcore,
            mutation: crate::presets::resolve_chain("grid-break").unwrap(),
            seed: 0,
            ..DemoConfig::default()
        };
        let preset = render_demo(&base);
        // Seed 0 means "no mutation", so it must equal the unmuted preset exactly.
        let unmuted = render_demo(&DemoConfig { mutation: Vec::new(), ..base.clone() });
        assert_eq!(
            fnv1a64_f32(&preset),
            fnv1a64_f32(&unmuted),
            "--seed 0 must reproduce the preset exactly (this is what pins the golden reference)"
        );
        let mut distinct = std::collections::BTreeSet::new();
        for seed in [1u64, 2, 3, 7, 0xA17E, 99] {
            let out = render_demo(&DemoConfig { seed, ..base.clone() });
            distinct.insert(fnv1a64_f32(&out));
        }
        assert!(
            distinct.len() >= 5,
            "six seeds produced only {} distinct renders — mutation is not reaching the audio",
            distinct.len()
        );
        assert!(!distinct.contains(&fnv1a64_f32(&preset)), "a mutated seed reproduced the preset");
    }

    #[test]
    fn the_same_seed_reproduces_the_same_render() {
        let cfg = DemoConfig {
            bars: 1,
            sample_rate: 24_000,
            channels: 1,
            style: Style::Glitch,
            mutation: crate::presets::resolve_chain("chaos").unwrap(),
            seed: 0xC0FFEE,
            ..DemoConfig::default()
        };
        assert_eq!(
            fnv1a64_f32(&render_demo(&cfg)),
            fnv1a64_f32(&render_demo(&cfg)),
            "ADR-007: the same seed must give the same samples"
        );
    }

    #[test]
    fn transpose_moves_the_bass_pitch() {
        let base = DemoConfig {
            bars: 1,
            sample_rate: 24_000,
            channels: 1,
            bpm: 150.0,
            delay_send: 0.0,
            solo: Solo::Bass,
            ..DemoConfig::default()
        };
        let a = render_demo(&base);
        let b = render_demo(&DemoConfig { transpose: 7, ..base.clone() });
        assert_ne!(fnv1a64_f32(&a), fnv1a64_f32(&b), "transpose had no effect on the bass");
        // A fifth up should raise the dominant frequency. Measured crudely by zero-crossing count,
        // which is enough to prove direction without an FFT.
        let crossings = |v: &[f32]| v.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!(
            crossings(&b) > crossings(&a),
            "transposing +7 semitones should raise the pitch: {} vs {} crossings",
            crossings(&a),
            crossings(&b)
        );
    }

    #[test]
    fn mutation_chains_all_render_without_panicking() {
        for (name, chain) in crate::mutation::preset_chains() {
            for style in Style::all() {
                let cfg = DemoConfig {
                    bars: 1,
                    sample_rate: 24_000,
                    channels: 1,
                    bpm: 150.0,
                    delay_send: 0.0,
                    style,
                    mutation: chain.clone(),
                    seed: 0xA17E,
                    ..DemoConfig::default()
                };
                let out = render_demo(&cfg);
                assert!(
                    out.iter().all(|s| s.is_finite()),
                    "chain `{name}` on {style:?} produced NaN"
                );
                let peak = out.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
                assert!(peak <= 1.25, "chain `{name}` on {style:?} exceeded headroom: {peak}");
            }
        }
    }

    #[test]
    fn glitch_polymeter_does_not_repeat_every_bar() {
        // The point of the 11-step hat lane against a 16-step kick: the composite should not repeat
        // on the bar line. Compare bar 1 against bar 2 of a four-bar render; if the polymeter were
        // broken (e.g. both lanes forced to 16), the bars would be identical.
        let cfg = DemoConfig {
            bars: 4,
            sample_rate: 24_000,
            channels: 1,
            bpm: 150.0,
            delay_send: 0.0,
            style: Style::Glitch,
            seed: 0,
            ..DemoConfig::default()
        };
        let out = render_demo(&cfg);
        let bar = out.len() / 4;
        assert_ne!(
            fnv1a64_f32(&out[..bar]),
            fnv1a64_f32(&out[bar..2 * bar]),
            "bars 1 and 2 are identical, so the lanes are not in polymeter"
        );
    }

    #[test]
    fn every_voice_is_audible_when_soloed() {
        // Regression test for the silent kick: `AdEnv::level()` was being read without `tick()`, so
        // the kick amp envelope sat at zero forever and the kick measured *exactly* 0.0 while the
        // bass and hat worked. A per-voice audibility check is the only test that catches a whole
        // voice being dead — the mix-level tests all passed with the kick missing, because the bass
        // and hat covered for it.
        for solo in [Solo::Kick, Solo::Bass, Solo::Hat] {
            let cfg = DemoConfig {
                bars: 1,
                sample_rate: 24_000,
                channels: 1,
                bpm: 138.0,
                delay_send: 0.0,
                solo,
                ..DemoConfig::default()
            };
            let out = render_demo(&cfg);
            let peak = out.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
            let rms = (out.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
                / out.len() as f64)
                .sqrt();
            assert!(peak > 0.02, "{solo:?} soloed is silent or near-silent: peak {peak:.6}");
            assert!(rms > 0.001, "{solo:?} soloed has no energy: rms {rms:.6}");
        }
    }

    #[test]
    fn solo_isolates_a_voice() {
        let kick = render_demo(&DemoConfig { solo: Solo::Kick, delay_send: 0.0, ..small() });
        let hat = render_demo(&DemoConfig { solo: Solo::Hat, delay_send: 0.0, ..small() });
        assert_ne!(fnv1a64_f32(&kick), fnv1a64_f32(&hat), "solo had no effect");
    }

    #[test]
    fn bass_level_zero_removes_the_bass() {
        let with = render_demo(&DemoConfig { bass_level: 0.55, ..small() });
        let without = render_demo(&DemoConfig { bass_level: 0.0, ..small() });
        assert_ne!(fnv1a64_f32(&with), fnv1a64_f32(&without), "bass_level had no effect");
    }

    #[test]
    fn crush_setting_changes_the_hat() {
        let clean = render_demo(&DemoConfig { hat_crush_bits: 24, ..small() });
        let crushed = render_demo(&DemoConfig { hat_crush_bits: 4, ..small() });
        assert_ne!(fnv1a64_f32(&clean), fnv1a64_f32(&crushed), "bit crush had no effect");
    }

    #[test]
    fn delay_send_changes_the_output() {
        let dry = render_demo(&DemoConfig { delay_send: 0.0, ..small() });
        let wet = render_demo(&DemoConfig { delay_send: 0.6, ..small() });
        assert_ne!(fnv1a64_f32(&dry), fnv1a64_f32(&wet));
    }

    #[test]
    fn channel_count_fans_out_identically() {
        let cfg = DemoConfig { channels: 4, bars: 1, ..small() };
        let out = render_demo(&cfg);
        for frame in out.chunks(4) {
            for s in frame {
                assert!((s - frame[0]).abs() < 1e-7, "mono patch must fan out identically");
            }
        }
    }
}
