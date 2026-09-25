# ADR-003 — UI renderer path: egui scaffold → custom WebGPU shell (decision D-1)

**Status:** accepted · **Date:** 2026-09-20 · **Related:** ADR-001, `design/token-spec.md`, `design/look-board.md`

## Context

The UI is not a skin on this project — it is the visual identity, the editing surface and the stage visual, all rendered by the same application at up to 4K on a projector. That argues for a bespoke renderer. But a bespoke renderer built before the audio engine exists argues for a year with no instrument. The decision must therefore sequence the two, not choose between them.

## Decision

**Phase 0–2: egui (immediate mode, `wgpu`-native) behind sparq's own abstraction. Phase 6: replace the drawing half with a custom retained-mode `wgpu` shell.**

Rules that make the swap cheap and the identity survive it:

1. **Design tokens are data, authored independently of any toolkit** (`design/tokens/*.toml`). A generator emits Rust constants *and* an SVG/CSS export for mockups. No widget code may contain a literal colour, size, spacing, radius or duration — enforced by a CI grep audit.
2. **One gesture model, owned by sparq.** Touch, pen and mouse are normalised into a single gesture API (tap, drag, long-press, two-finger pan/pinch, second-finger fine-resolution, multi-touch tracking). egui is a *consumer* of that API in Phase 0–2; the custom shell consumes the same API in Phase 6.
3. **One layout contract.** Panels, rail, dock, breakpoints and the 8 px grid are specified in `docs/ui/*` and implemented behind an interface, so the shell swap does not re-decide layout.
4. **Display modules are separate from widgets.** Scope/spectrogram/attractor/rig-map rendering is GPU work behind a `gpu` port and a draw-interface (plan §6.3, §14.4). It is written once and survives the swap untouched.
5. **Escalation trigger:** if egui cannot deliver the gesture model or touch ergonomics by the end of Phase 0 week 5 (WO-012/WO-013 acceptance criteria), the custom shell is pulled forward and this ADR is superseded — recorded with evidence, not vibes.

## Consequences

* Sound and music arrive in weeks, not months; the identity is validated with static mockups (WO-004) and then with a real, if temporary, shell.
* Some Phase 0–2 UI work is throwaway (egui style adapters, panel plumbing). Accepted: it is small, and the tokens/gestures/layout specs are not.
* Immediate mode is the right call for a canvas with hundreds of live-updating elements during exploration; retained mode is the right call for the final 2 000-node graph at 4K. Sequencing them matches their strengths.
* A screenshot-diff test suite starts in Phase 0 (against egui) and must be re-baselined at the Phase 6 swap — budgeted in that phase.

## Alternatives considered

* **Custom `wgpu` shell from Phase 0.** Best final result, worst time-to-music; risks 6–10 extra weeks before a sound. Rejected for sequencing, not for quality.
* **A full toolkit throughout (iced / Slint / GPUI-class).** Fastest overall, but inherits someone else's widget language — the "scientific instrument" look and the 4K stage visuals both become fights with the framework. Rejected.
* **Web tech (Tauri/Electron + canvas/WebGL).** Excellent UI velocity, but a browser compositor in the same process as a real-time audio callback is a latency and reliability risk on stage, and 4K GPU trace rendering would be second-class. Rejected for the instrument; **retained** for the optional thin client (D-10, Phase 6).
* **JUCE/Qt UI on a Rust core.** Two languages, FFI friction, and neither gives the touch-first scientific look without heavy custom work. Rejected.

## Review trigger

End of Phase 0 (go/no-go with evidence), and again at the start of Phase 6 before any shell code is written.
