// sparq instrument template — source starter (Rust + sparq-module-guest SDK, WO-017)
//
// THIS FILE IS SOURCE, NOT PACKAGE. The distributed five-file package carries the compiled
// `template.wasm`, not this file. Build: `cargo component build --release` (or your
// language's component-model toolchain) → `sparq mod package` → validate → drop in instruments/.
//
// The contract you are implementing is the same one first-party modules implement
// (docs/module-author-guide-v0.md §3): prepare allocates, process does not, everything
// else has a default. The guest SDK re-exports it over the WIT binding.

use sparq_module_guest as sparq; // v0.1 — published SDK; the core workspace does not depend on it
use sparq::{AudioCtx, BlockStatus, DisplayList, DrawCtx, ModuleError, Resources, TokenId};

pub struct TemplateInstrument {
    freq: f64,
    amp: f32,
    phase: f64,
    last_gate: bool,
}

impl sparq::Module for TemplateInstrument {
    fn id(&self) -> &str {
        "you/syn/template" // must equal identity.id exactly — a lie is a registration refusal
    }

    /// ALL allocation happens here. Called again whenever Resources change; write it idempotent.
    fn prepare(&mut self, _r: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }

    /// No allocation, no locks, no I/O, no clock, no panics. Fuel is metered; blow the budget
    /// and the host auto-bypasses you (with words in the UI) — the set continues without you.
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // Param snapshot: version + normalised f32 values in MANIFEST DECLARATION ORDER
        // (ParamSet alignment — bool rides 0.0/1.0, enum its option index; text/blob params
        // never ride the snapshot, they arrive via configure/message). Same version ⇒ same
        // values, so you may skip re-reading.
        let params = ctx.params();
        self.freq = f64::from(params.at(0)); // "freq" — [[params]] index 0 in sparqmod.toml
        self.amp = params.at(1); // "amp"  — index 1

        for (frame, events) in ctx.frames_with_events() {
            for ev in events {
                // Flat Copy events, native shape: kind/sample/channel/value/words[4] —
                // trigger intensity is ev.value, sample offset is ev.sample.
                if ev.kind == sparq::EventKind::Trigger {
                    self.phase = 0.0; // sample-accurate onset
                }
            }
            let s = (self.phase * core::f64::consts::TAU).sin();
            self.phase = (self.phase + self.freq / ctx.sample_rate()).rem_euclid(1.0);
            frame.out[0] = (s * self.amp) as f32;
        }
        BlockStatus::Ok // Ok | Silenced | Overrun | Failed — the native four, unchanged
    }

    /// Per-frame draw export: emit STRUCTURE, never appearance. Every style is a semantic
    /// token id resolved by the host against the current token bundle — this is why an app
    /// design change re-themes your instrument with zero intervention (ADR-010).
    fn draw(&self, ctx: &DrawCtx<'_>) -> DisplayList {
        let mut list = DisplayList::new();
        list.hairline_rect(ctx.frame, TokenId::STROKE_HAIRLINE_FAINT);
        // A one-period waveform of the current freq, as a data trace in the audio signal colour:
        list.trace(
            &ctx.waveform_points(self.phase, self.freq, 128),
            TokenId::COLOR_SIGNAL_AUDIO,
            TokenId::STROKE_REGULAR, // width class 2 — strokes are 1, 2 or 3, always
        );
        // No literal colours. No pixel drawing. No custom shaders. That is the whole rule.
        list
    }

    fn configure(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
        Ok(()) // honest refusal is valid; silently ignoring a message is not
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("none"))
    }
}

// Tests travel with your source project, not in the package:
//  * the golden render case declared for `sparq mod validate` (bit-exact, twice);
//  * the real-time property test (no allocation in process — module guide §10);
//  * a fuel-budget assertion under your declared max_fuel.
