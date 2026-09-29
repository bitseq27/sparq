# WO-012 increment 4 — the device blocker and the operator's four asks (plan of record, 2026-09-30)

The operator applied every bundle, opened the window on SATURN, tapped PLAY, and got silence
plus the sentence that names this increment:

> `PLAY REFUSED: enumerating wasapi-shared devices: system error: this thread already
> initialised COM as single-threaded (STA); the WASAPI HAL needs MTA. Phase 0 hosts are console
> apps — if this appears inside a GUI host, the shell (WO-012) must not OleInitialize on the
> audio-control thread`

with three further asks in the same message: **out/main meters like the mockup**, **mouse
usability** (the log's `motion for an untracked pointer (missed Down?)` lines are mouse hover
being suppressed as broken touch), and **smaller, coloured, grouped dock tiles**. One
increment, four declared items — the WO-013 inc 5 "Else column" precedent. Decisions before
code.

## D1 — The audio-control thread (the blocker).

The HAL's contract (wasapi.rs header): every control-side call runs inside a **persistent
per-thread MTA** (`ensure_control_com`, three call sites: discover, open, and the stop path).
A console host's main thread is uninitialised, so `sparq play` gets MTA and works. A winit
host's UI thread is **STA** (the windowing stack OLE-initialises it), and an STA thread can
never become MTA — so no HAL call may ever happen on it. The fix the error message prescribes,
made structural: **`LiveSession` spawns one dedicated audio-control thread per session**; it
is the only thread that touches the backend (enumerate → probe open → real open → start →
stop → drop → pump/fault/capture in the manual null case). It initialises MTA through the
HAL's own door by simply *calling the HAL* (non-Windows: an ordinary thread). The UI thread
keeps what is thread-safe by construction: the `SharedEngine` control half (its rings and
hot-swap are `Send + Sync` by design), the node map, the drain scratch.

Protocol (bounded, blocking replies on a control path — never on the audio path):
`Open{opts, request}` → reply `Negotiated{cfg, latency summary string, device name}` or
`Err(sentence)`; the UI thread then builds the executor and sends `AudioEngine` with
`Start` → reply `Started`/`Err`; `Stop` → reply the measured evidence parts (diag snapshot
fields, error report state + last error) and the captured buffer if any; `Pump(n)` →
reply count; `Fault(f)` → reply; `Health` is NOT a command: the worker mirrors the stream's
state atomically and last error into an `Arc` pair the session reads per frame (the state is
an `AtomicU8` inside the stream; the worker polls it on its command loop's wake-ups and on
every reply — cheap, and the per-frame health gate keeps its one-honest-line behaviour).
Session drop sends `Stop` and joins (a session that leaks its thread leaks the device).
The audit's manual-null path rides the same protocol (`Pump`, `Fault`, capture in the `Stop`
reply AND a `Capture` command for the mid-session assertion) — determinism is preserved
because the null pump is synchronous inside the worker and the reply carries the count.

What this buys beyond the fix: the shell becomes apartment-proof for any future host (a
Phase 6 shell, a service, a plugin wrap), and `stop`'s blocking join moves off the UI thread
into the session's own teardown.

## D2 — Mouse usability (the log's suppressed hover lines are the symptom).

* **Hover is not a broken tap.** `window.rs` tracks button state; `CursorMoved` is forwarded
  to the recogniser only while a button is held (a drag). Unpressed motion is dropped at the
  adapter — the suppression log line can then only mean a real missed Down, which is what it
  was for. Declared parked: hover affordances (cursor shapes, hover highlights) — §8 forbids
  hover-*only* affordances anyway, so nothing touch-first is lost.
* **Right button = the context menu, immediately.** `MouseInput(Right, Pressed)` maps to
  `GestureIntent::Context { pos }` through a new `FrameInput::extras` slice (the window
  synthesises what the recogniser would have concluded after 350 ms of finger — the menu is
  the same menu, the intent vocabulary is unchanged). Touch keeps the long-press; the mouse
  gets the button desktop users already hold in their hand.
* **The wheel scrolls what is under it.** New `GestureIntent::Wheel { pos, delta }` (emitted
  only by the window adapter and by smokes — the recogniser never invents it): over the
  inspector panel it scrolls the rows (the two-finger pan's routed twin, same clamp, same
  honesty about hidden rows); over the canvas it pans the camera; over the dock it is
  declined in words for now (the dock has no scrollable content yet — cards overflow into a
  count line, not a scrollbar). No modifier keys anywhere (§8: nothing requires a modifier).
* Headless parity: `extras` is a slice in `FrameInput`, so smokes drive right-click and wheel
  through the same door the window uses — both get a smoke.

## D3 — out/main meters, the mockup's bars (green level, amber peak-hold).

* `MeterUpdate` grows `peak_l` / `peak_r` (the first two channels' peaks; a mono port
  duplicates; the `FOLDED` entry of a cv-only node keeps honest zeros). The publish pass
  already walks every output buffer per port — the per-channel split is one more accumulator
  in the same cache-warm pass; the folded `peak`/`rms` and every existing consumer are
  untouched (additive fields, pinned by the existing bit-identity test which now also pins
  `peak_l == peak_r == peak` for the mono reference node).
* The **stereo bars** draw on `out/main` (the operator's node; `OUT_MAIN_ID` is the named
  constant): two 6 px wells in the node body, green **data-class** fill (a meter is data
  *about* audio — the mockup's green, §4-consistent under that reading; recorded as a
  finding), width = the channel peak, plus an amber **peak-hold block** per channel decaying
  on AUDIO time (half-life `motion.toml`'s 300 ms expressed in blocks at the negotiated rate —
  no wall clock on the display path). At rest: empty wells, never a frozen bar.
* The bars read the session's per-frame drain (a small stereo map beside the traces — same
  single-source discipline, D3′ of inc 6: at rest the painter gets an empty map). RENDER WAV
  does not feed them (the offline preview publishes no ring) — declared, like the scope's
  rest state.
* Mockup geometry (118×6 and 104×6 bars at the node's left inset) is the reference; sizes ride
  space tokens.

## D4 — Dock tiles: smaller, coloured by what they carry, grouped by what they are.

* Tiles shrink from 248×80 to **112×48** (token values moved through the generator; the touch
  class moves L → M, still ≥ 44 px, and the mockup-review touch table row is amended by a
  finding). More tiles fit: at 1920 the dock shows the whole first-party seventeen in two
  rows, which is the point — the palette you reach for should be the palette you see.
* **Colour = the dominant SIGNAL class** (the tile's left edge stripe, 2 px signal weight):
  amber sources/fx/out (audio out), cyan mod/env (cv out), green ana (data), violet dsp
  spatial… exactly `class_colour(dominant class)` — §4's rule, not accent-by-category (which
  §8 forbids): the stripe says what the module *carries*, the group says what it *is*.
* **Groups by the manifest's top category** (`syn`, `flt`, `fx`, `mod`, `env`, `ana`, `util`,
  `out`, `dsp`), each under its xs word, tiles flowing in rows inside the group; groups stack
  in registry order. The overflow line stays for whatever does not fit.
* Card content reduces to stripe + name (xs) — the category line goes away (the group header
  now says it); version goes away (the registry is single-version in Phase 0).

## Postscript — measured, and what the smoke taught (2026-09-30)

**Gates on the final tree:** fmt clean · clippy clean in every runnable cell (three lint debts
paid in words: the reasoned control-path `Mutex` allow, `checked_div`, the boxed command
variant) · **777 tests** (+2 session gates) · **37 smokes** · matrix 0 violations · 5 python
gates · release goldens bit-identical (stress `7bb06379bd6845e5` debug AND release, exec
renders, `canvas-render.wav`, determinism) · selftest 9/9 · 17/17 · probe 0/5 000 · stamp
`src 94f/2188595B` · sealed `sync-wo012-inc4.zip` (23 entries).

**The wheel smoke failed once, and the failure was the design working:** it first wheeled over
the demo sine's inspector — two params, nothing to scroll, `max_scroll == 0`, the clamp honest.
The smoke now spawns the 24-param mixer from its dock card first (proving that door too) and
scrolls "its" inspector. A scroll gesture over a panel with nothing to scroll still moves
nothing — increment 5's honesty rule, unrelaxed.

**Hover silence is asserted where it lives:** the adapter gate is in `window.rs`, which the
sandbox cross-lints but never runs; the smoke therefore asserts the recogniser's side (a
synthetic untracked Moved still suppresses, for touch) and the adapter's side is stated in its
comment and proven on the device by the absence of the old log spam. Declared, not hidden.

## Proof burden

* **The blocker, proven where it lives:** a sandbox test cannot be STA — but the protocol can:
  the session tests all ride the worker now (open/pump/capture/stop/unplug unchanged in
  meaning); a new test starts TWO sessions sequentially (thread teardown hygiene: no leaked
  thread, no leaked device — the second open proves the first dropped clean) and a new test
  drops a session without stopping (Drop sends Stop and joins). On SATURN the proof is the
  operator tapping PLAY and hearing sound — test006 J.
* Mouse: two smokes (right-click opens the node menu where a long-press would; a wheel over
  the inspector scrolls rows and over the canvas pans, with the hidden-row honesty intact) +
  the hover-spam gone (the window no longer forwards unpressed motion — asserted by the
  absence of new suppression lines in a mouse-move frame sequence in a smoke).
* Meters: a session test (fed out/main publishes stereo peaks: `peak_l`/`peak_r` > 0 and the
  mono pin `peak_l == peak_r == peak` on a mono node; the hold decays on block count, not
  frames) + a smoke (after PLAY+pump the out/main wells are non-empty in the session's meter
  map; at rest they are empty).
* Dock: the existing dock-card smoke still spawns by tap at the new size (class M in the
  audit, 0 violations); a smoke asserts the group headers exist in order (state: the dock's
  computed group list, exposed for tests) and that all seventeen tiles fit at 1920×1080.
* Standing gates: 777+ tests at the new count, 34+ smokes, every golden bit-identical (no
  render path moves — publish_meters grows fields, the executor's audio is untouched), stress
  hash unchanged, clippy cells incl. MSVC `ui-window` (the window adapter changes cross-lint),
  the 5 python gates, token regeneration clean, stamp + log_check BASELINE moved at seal.

## Out of scope (declared)

Hover affordances and cursor shapes; dock wheel-scroll (nothing scrollable yet);Perform-mode
mouse conventions; per-channel meters on every node (out/main first, as asked); the inspector
response plot and node inset displays (increment 5, the convergence's slice B); clipboard and
keyboard shortcuts beyond the modal sheets.
