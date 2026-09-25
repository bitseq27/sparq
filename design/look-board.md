# sparq look board — the conformance authority

**Version:** 0.1.0 · **Work order:** WO-002/WO-004 · **Related:** `token-spec.md`, plan §3.3/§14.5
Any new surface — widget, panel, display module, custom module draw routine — must be able to sit next to this board without obvious dissonance. When tokens and this board disagree, **this board wins**.

---

## 1. Identity in one paragraph

sparq looks like laboratory instrumentation that happens to make music: a near-black ground, hairline structure, phosphor traces that glow only where signal flows, and tabular monospace numerals with SI units. It is drawn, not decorated. Depth comes from tint and line weight, never from shadow. Motion is critically damped and brief. A screenshot with no title should read as *scientific software*.

## 2. Reference frame

Oscilloscope · spectrum analyser · seismograph · technical drawing / blueprint · astronomical plate · laboratory data logger. **Not:** consumer music apps, skeuomorphic hardware, gaming HUDs, "neon cyberpunk".

## 3. The stroke language

| Weight | Meaning |
|---|---|
| 1 px | **Structure.** Panel borders, dividers, node frames, port rings, grid, inactive wires, axis ticks |
| 2 px | **Signal.** Active wires, live traces, meter fills, focused element |
| 3 px | **Emphasis.** Selection frame, hero trace in a full-bleed visual, the transport playhead |

Rules: structure is never filled, signal is never hatched, emphasis is rare. If everything is 2 px, nothing is.

## 4. Colour discipline

* Five accents, one per **signal class**: amber = audio, cyan = control/CV, magenta = events, green = data, violet = spatial. A colour always tells you *what kind of signal this is*.
* Structure and text are neutral cool white in three tiers. Never tint structure.
* States (bypassed, muted, stale, error, stub) use **pattern first**: hatch, dash, gap-dash, jagged underline, crosshatch. Colour is redundant.
* Glow is data: bloom radius and intensity follow amplitude (`motion.toml [trace]`). Silence looks silent.
* Forbidden: pure white, drop shadows, gradient decoration, arbitrary colours, accent-by-category.

## 5. Numeric discipline (the most recognisable part of the identity)

* Tabular monospace numerals, always. Values right-aligned in columns; units immediately after, at 0.85 em, in `text.tertiary`.
* A number without a unit is a bug. Precision is fixed per unit (`typography.toml [format]`) so columns don't jitter.
* Signed quantities always show their sign (`+3.2 dB`, `−12 ct`).
* Scientific notation and SI prefixes above 10 000 (`12.4 kHz`, not `12400 Hz`).
* Musical position as `bar.beat.tick`. Clocks as `hh:mm:ss.mmm`.
* Big numbers in Perform mode: `type.scale.stage` (44 px) minimum, hero numerals 88 px, legible at 3 m.

## 6. Composition

* 8 px grid, snap everything. Panels separated by hairlines and tint steps, not by shadows or gaps-with-borders.
* Density is a feature: this is an instrument for someone who wants 40 parameters visible. But density comes from **small type and hairlines**, never from cramming touch targets — targets stay ≥44 px, spacing absorbs the difference.
* Empty space is structure: leave the canvas ground visible. Never fill a panel because it looks empty.
* Alignment: labels left, values right, units flush. No centred text except in Perform-mode hero readouts.
* Iconography: geometric, 1 px stroke, always paired with a label outside Perform mode; inside Perform mode icons must be unambiguous at a glance (transport, panic, record are the only permitted icon-only controls).

## 7. Motion

Critically damped, 120–180 ms for UI, never bouncing, never elastic. Traces redraw per frame with optional phosphor persistence. Transport-driven motion is linear (a playhead must not ease). Scene morphs are musical durations, not UI durations.

## 8. Forbidden list (a build error or a review rejection)

Drop shadows · rounded corners >4 px · skeuomorphism (wood, metal, leather, screws, VU needles) · gradient fills as decoration · glow not tied to amplitude · arbitrary colours · accent assigned by module/category · icon-only controls in Design mode · hover-only affordances · anything requiring a modifier key · touch targets <44 px · pure white · bounce/elastic easing · decorative animation · centred body text · numbers without units · colour as the only encoding of a state.

## 9. Conformance checklist (paste into every module/panel review)

1. Does it use only tokens? (grep audit clean)
2. Are stroke weights from {1, 2, 3} and used per §3?
3. Is every colour traceable to a signal class, a state, or a text/structure tier?
4. Does every state have a non-colour encoding?
5. Are all numerals tabular, unit-suffixed, right-aligned?
6. Are all interactive elements ≥44 px (or explicitly badged as a Design-mode dense exception)?
7. Is it legible at 200 % type scale and in the high-contrast theme?
8. Does it hold at 3 m in a dark room (Perform-mode surfaces)?
9. Does it sit next to the mockups without visible dissonance?
10. Would it pass the blind test (§10)?

## 10. Test protocols

**Blind identity test.** Show a full-resolution screenshot, no title, to someone who doesn't know the project. Ask "what kind of software is this?" Pass if the answer lands in {scientific, laboratory, engineering, measurement, research, instrumentation}. Fail if it lands in {music, game, consumer, DJ}. Record the verbatim answers in `mockup-review.md` — over time these become the most honest metric of the identity.

**Distance test.** Perform-mode surfaces viewed at 3 m on the target projector/display: every primary numeral readable, every macro pad identifiable, no accidental legibility of Design-mode chrome.

**Dark-room test.** In near-darkness with a single practical light: no blooming whites, no lost hairlines, states still distinguishable by pattern.

**Glove/sweat test.** On the stage device: connections, long-press menus and macro pads work with a thin glove and with damp fingertips; palm rejection holds while drawing a wire across the canvas.

**Monochrome test.** Convert the screenshot to greyscale. Signal classes must remain distinguishable by weight/dash, states by pattern, and the hierarchy must survive. If it collapses, colour is carrying structure it shouldn't.
