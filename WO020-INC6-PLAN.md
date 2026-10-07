# WO020-INC6-PLAN.md — "the live instrument": the launch wiring (INC5b, discharged here) + the operator's window round

**Commissioned 2026-10-07 by the operator's device report after r7** (the hand-in gate had just
gone COMPLETE + PASS on SATURN; the wall was on the canvas for real for the first time). The
report, verbatim: *"the observatory does not have any controls or settings on the window, the
window is too big, only display 4 panels, the panels are not updating, i have a nasa api key that
will allow 1000 calls per hour, have a field to add an api key and also a poll rate setting for
each panel."*

Every clause is a fact of the shipped state, not a defect: r7's honest state 1 said the shell
shows the AT-REST wall (the loader is not wired into launch — LATER.md's INC5b), the card is the
declared `min_size` fixed (plan D6), the toolbar exists only as manifest data
(`[[ui.panel.widgets]]`, the panel band reserved but untyped — layout.rs's own comment), keys and
cadences are env-and-registry only, and the default face IS the 16-cell wall ("only display 4
panels" is the 2208×1288 card at a readable zoom: about four cells fit a viewport). This increment
turns the picture on the wall into the instrument on the wall. It SUPERSEDES the INC5b door in
LATER.md (launch wiring = slice S4 here); the door's three named parts (discovery → registration,
PLAY #58, the live-provider swap) are all discharged inside this plan.

## 0. The operator's rulings (2026-10-07, recorded verbatim as chosen)

| # | Ruling | The choice |
|---|---|---|
| **O-1** | the window | **Resizable card, default HALF size** — drag-resizable; default 1088×560 world px band (fits a 1080p viewport at zoom 1; FOCUS frames the whole wall); the manifest's 2176×1120 stays the declared maximum; the live guest re-renders at the ACTUAL band size so text stays native, the at-rest render scales |
| **O-2** | keys + rates placement | **STREAMS dock tab + card rate fields** — the dock's disabled STREAMS tab comes alive (one row per stream: live status, cadence, KEY field); the card's per-panel rate field writes the SAME per-stream override (panels sharing a feed share its rate — the broker's truth, said in words) |
| **O-3** | poll-rate floor | **10 s hard floor** — registry cadences (60–1800 s) stay the polite defaults; a panel may override its stream anywhere from 10 s upward. The operator's 1000-calls/h NASA key covers 10 s polls on the three NASA feeds even together (3 feeds × 360/h = 1080/h at the absolute worst alignment; the one-retry-on-429 policy and the attempt-counting cadence floors absorb the overlap) |
| **O-4** | key storage | **User-data file, redacted** — keys persist beside the streams cache in the OS user-data dir; env vars take precedence; the house redaction discipline rides along (a resolved key exists only inside `Request::url`, never in logs, patches, the journal or the at-rest artefacts). Same threat model as the env var itself; the OS credential store was offered and declined (non-Windows fallback would need the file anyway) |

## 1. Design decisions (D-numbers continue the WO-020 series)

* **D15 — the card size is canvas state, per node, instruments only.** `Node.size:
  Option<Vec2>` (world px, the CARD size; `None` = the layout's default for the spec). No patch
  format exists yet (the `.sparqpatch` files are package artefacts; the shell has no
  loader/saver — SCENES is a disabled word), so there is no format change to make: the field is
  representable and undoable from day one, and `docs/formats/project.md` grows the reserved
  optional `size = [w, h]` node line NOW so the format lands with the door already cut. Clamp:
  the band stays ≥ 480×248 (a quarter of the declared face — below that the wall is words, not
  data) and ≤ the declared `min_size` (2176×1120 — the guest never renders above its declared
  face; O-1). The chrome (gutter/header/panel band) rides the size exactly as D6 computes it.
  The resize affordance: the selected instrument card grows a corner handle (bottom-right, class
  L touch target, registered in the audit like every command surface); the drag snaps to the
  8-px grid on commit; one `Op::ResizeNode` per drag (the cable-node precedent: one undo per
  gesture). FIT/FOCUS/ARRANGE read the size like any card.
* **D16 — the panel band types the manifest's widgets, and nothing else.** The guest's toolbar is
  manifest data (`[[ui.panel.widgets]]`: row 0 = CELL + the sixteen STREAM pickers gated
  `visible_if {param=selected_cell}` — one visible at a time, reading as a single dropdown, D7;
  row 1 = LAYOUT / SOLO / FULL / PAUSE / TICKER). `NodeSpec::display` grows the widget list
  (today it keeps only the row COUNT); the card paints the declared widgets into the reserved
  band at the 44-px touch floor; enums open the EXISTING picker (§8.4's sheet); toggles and
  buttons ride `Op::SetParam` (undoable, journal-safe). The host paints no control the manifest
  does not declare — an instrument's face is its package's, per ADR-010's runtime-token rule.
  The HOST furniture (D18's rate row) is visually distinct and lives below the guest's rows, on
  the `out/main` driver-info-band precedent: host words in the card, never guest widgets.
* **D17 — the live plane is three swaps and a thread, behind the features that already exist.**
  (a) LAUNCH: `ShellUi::new` (feature `instrument-host`) loads each discovered package through
  `InstrumentRuntime::load_package` and registers it via the registry's `register_instrument` /
  `SharedFactory` door — the executor then adopts it like any module and PLAY's refusal (#58)
  lifts for port-less instruments; a package that refuses to load is WORDS in the log and an
  absent browser row, never a half-instrument. (b) PROVIDER: the shell's `ReplayProvider` swaps
  to a `BrokerProvider` whose driver thread (control plane, feature `streams-net`) polls the due
  streams through `HttpTransport` on the injected clock's cadence — the broker core is the
  hermetic half that is already tested; the thread is the `ui/live.rs` `Arc<Mutex<…>>` idiom,
  never an audio path. The last-good cache seeds relaunches (broker rule). (c) DISPLAY: the UI
  thread owns each instrument's display instance and calls `draw(frame-context)` at the DISPLAY
  CADENCE (D19) with the band's actual px, the camera's LOD and the animation clock; the returned
  surface becomes the band's live display list (the at-rest store stays the fallback — "live
  where the instance carries it, at rest otherwise", the meters' rule extended). Fuel budget =
  the declared `max_fuel` per call; an overrun skips the frame and counts; five consecutive
  bypass the DISPLAY instance with §3's words in the panel band — the audio instance is
  untouched (decision D's isolation). The frame-context's `time_sec` is the shell's animation
  clock (legal: a display is never replayed — D8), so the ticker scrolls and PAUSE freezes it.
* **D18 — keys and cadences are one host-side store, per stream, in the user-data dir (O-2/O-4).**
  A small TOML beside the streams cache (`sparq/streams.d/overrides.toml`): per stream id,
  `key` (string, optional — env WINS over the file, so a machine policy cannot be edited by the
  instrument) and `cadence_s` (u32, optional — floor 10 s per O-3; below the floor the field
  refuses in words and keeps the old value). The broker's env closure reads key-store-then-env;
  `Broker::due` reads the override-then-registry cadence (the cadence floors still count
  ATTEMPTS — the anti-thundering-herd rule is untouched). The store is control-thread-only I/O,
  loaded at launch, written on edit, and NEVER read on an audio path. Redaction: the key value
  appears only inside `Request::url` (the fetch module's existing rule); the UI masks it
  (`••••last4`), the log prints the KEY STATE words (`SET (file)` / `SET (env)` / `UNSET` /
  `KEY NEEDED`), never the value.
* **D19 — the display cadence is a token, default 15 Hz, plus an immediate draw on edits.** A
  full wall draw measured 12.4 M fuel warm (~ms-class on SATURN, unmeasured); 60 Hz guest draws
  would tax the UI thread for a data wall whose fastest feed is a 5 s cadence. 15 Hz keeps the
  ticker scroll smooth enough and the frame budget ≤ ~7 % of a 60 Hz frame; any param edit, cell
  swap, resize or LOD change forces the next frame's draw regardless of the pace (the edit's
  effect is immediate, the ambient scroll is paced). The token lives in `layout.toml`
  (`instrument_display_hz = 15`) — movement is an operator ruling like every token.
* **D20 — the STREAMS dock tab is the stream plane's window, and it is honest about liveness.**
  One row per registry stream: LED + word status (LIVE/STALE/OFFLINE/KEY NEEDED — the broker's
  own derivation at the injected now), the last-fetch age in words, the cadence (the override if
  set, dimmed registry value beside it; editable field, floor 10 s), and the key state + KEY
  field (masked; typing writes the store; env-set keys show `SET (env)` and the field is
  disabled with those words — the precedence is visible, not a mystery). The rows read the
  broker's health mirror per frame (the level-publication pattern); with no driver thread
  (sandbox, no `streams-net`) the tab shows the REPLAY provider's fixture statuses and says
  `AT REST — fixtures` in the header, never a frozen lie. The card's per-panel RATE field (D16's
  host row) writes the same override the tab does — one store, two doors, the words say which
  stream a panel's rate governs.

## 2. Slices (each lands sealed: tests, docs, the state card)

* **S1 — the resizable card (O-1/D15).** Sandbox-provable end to end: `Node.size` + the op +
  the layout + the corner handle + the clamp + the grid snap; the at-rest band scales; the
  default instrument card is 1088×560 band (half face); headless SVG + audit rows + interact
  tests; `project.md` grows the reserved `size` line. Acceptance: the whole 16-cell wall is
  readable inside a 1920×1080 viewport at zoom 1 after SPAWN + FOCUS; a resize is one undo;
  the audit registers the handle at the touch floor.
* **S2 — the typed panel (D16).** The widget list rides the NodeSpec; the band paints CELL /
  STREAM-dropdown / LAYOUT / SOLO / FULL / PAUSE / TICKER from the manifest; enums open the
  picker; every control is an undoable `SetParam`. Sandbox-provable (svg + audit + interact);
  at rest the wall does not visibly change (no runtime) — the card SAYS SO in the panel band
  ("params edit; the wall re-renders live when the loader runs" — refuse-in-words, never a
  frozen lie). Acceptance: every declared widget is painted and routed; an undeclared control
  exists nowhere.
* **S3 — the store + the STREAMS tab (O-2/O-3/O-4, D18/D20).** The overrides store (file,
  precedence, redaction, floor), the broker's due()/env reads through it, the dock tab with its
  rows and fields. Hermetic sandbox tests over a mock transport + a temp-dir store; the device
  gets the live rows. Acceptance: entering the NASA key flips neo/epic/power from 429/DEMO_KEY
  to the key's rate on the device; a 10 s override is honoured; a 5 s entry refuses in words;
  the key never appears in any log, patch or at-rest artefact (a redaction test greps for it).
* **S4 — the live plane (D17/D19): discovery → registration → adoption → the driver thread →
  the paced display draw.** The wasmtime half is device-first-compile (the §8 wall stands); the
  host-side seams (the provider swap, the draw pacer, the frame-skip/bypass counter, the
  live/at-rest band switch) are sandbox-tested behind traits. Acceptance on the device: PLAY
  with the Observatory on the canvas runs (the #58 refusal is GONE); the wall's cells update as
  the broker fetches; the ticker scrolls; PAUSE freezes it; a cell swap via the STREAM dropdown
  re-renders within a frame; the fuel watchdog's words appear if a draw overruns (inject by
  shrinking the declared budget in a copy — a gate nobody has seen fail is a gate nobody
  trusts). `test008.bat` is this slice's run sheet.
* **S5 — the card's rate row + convergence (D20's second door).** The per-panel RATE field in
  the host row, its words naming the governed stream, the shared-feed truth; the convergence
  sheet re-shot with the live wall; the mockup audit re-run.

## 3. Honest states (what this increment does NOT do)

* No audio: the instrument stays port-less (D5) — the wall is silent-but-live; sonification is
  the later increment the README already names.
* No per-cell cadences: the broker fetches per STREAM; two panels on one feed share its rate
  (O-2's words, said in the UI).
* No key vault: O-4 chose the user-data file; the OS credential store stays parked in LATER.md.
* The sandbox proves everything except the wasm runtime itself; S4's runtime half is the
  device's first compile, exactly like INC5's loader was.
