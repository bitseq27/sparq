# CHECKLIST.md — session handoff (live document)

**Purpose:** the first file a new session reads. Says what is done, what is in flight, and what to do
next. Updated at the end of every session (and mid-session when state changes). This file supersedes
`RESUME.md`'s handoff role (RESUME.md is the 2026-09-22 snapshot; keep it as history). For deep
history see `PHASE0-WORKORDERS.md` §2.1 (status table) and its build-log sections — read **by line
range**, never whole (see `RESUME.md` §1 for why).

**Last updated:** 2026-10-08, the INC6 session 9 (fresh sandbox on the re-published main
`b2445d6` — the operator's r8-seal re-publish; the recovery note is `WO020-STATE.md`'s INC6
section, first paragraph) — **WO-020 INC6 slices S1–S5 CLOSED — the work order's SANDBOX work
is COMPLETE: the resizable instrument card (ruling O-1, plan D15), the typed panel band (plan
D16), the overrides store + the STREAMS dock tab (rulings O-2/O-3/O-4, plan D18/D20), the live
plane's host-side seams + the device-first launch/driver (plan D17/D19), and the card's RATE
row + convergence (D20's second door). S4's wasmtime half is DEVICE-FIRST-COMPILE with
`scripts/test008.bat` as its run sheet; what remains of INC6 is the DEVICE round**. Landed (full
record in `WO020-STATE.md`'s INC6 section): `Node.size: Option<Vec2>` + `Op::ResizeNode` (ONE
undo per drag carrying the gesture's original `from`; the clamp lives in the op constructor —
band ≥ 480×248, ≤ the declared face, NaN sanitised, a no-change release records nothing); the
HALF-face default (the Observatory's default card **1120×728** / band **1088×560**; the
declared maximum 2208×1288 stays reachable through a resize and keeps the D6/R3 coverage rule
at 97.5%); the corner handle (bottom-right of the SELECTED instrument card, class-L capture,
painted in tokens, registered `canvas/resize/{id}`, offered only while drawn — never at Dot
LOD, never covered); the 8-px snap on commit; FIT/FOCUS/the marquee/the layout pass all read
the size like any card; the at-rest wall scales into the resized band (the INC4 painter's
existing uniform scale); `ShellUi::audit_element` (the audit-side twin of `rect_of`);
`docs/formats/project.md`'s reserved `size = [w, h]` node line; the audit rows re-cut for O-1
+ smokes **55/56** through the real recogniser (corner drag → one undo → three-finger restore;
SPAWN+FOCUS frames the 16-cell wall in a 1920×1080 viewport at a Full-LOD zoom ≤ 1 — the O-1
acceptance, measured). **Gates:** root `cargo test --workspace` **1047/0** (the r8 baseline
1034 + 13 new); `-p sparq-app --features ui` **45/1** (THE known sandbox-only env red,
`ui::live::…level_follows`); streams(+net) **96/0**; observatory **124/0** + goldens 6/6;
fmt/clippy `-D warnings` CLEAN (workspace, ui, streams-net cells); `ui --audit` **PASS (0
failures, 71 rows, was 64)**; the python gates green except `token_audit`'s standing §18
mockup red (14 groups/298 — the IDENTICAL set, zero new). Proven failable by three injections
(no-snap / no-registration / full-face-default), each reverted byte-identical. One targeted
clippy allow recorded with measured provenance (`ConnectOutcome`'s `large_enum_variant`: the
ruled `Node.size` grew `Node` 184→200 B, three bytes past the lint's threshold; boxing the
public `Op` was judged the disproportionate patch — the numbers and the reasoning are in
STATE). MUST-NOT-MOVE surfaces untouched (WIT, `modules/`, the stamp, fixtures, goldens).
S2 landed on top (full record in `WO020-STATE.md`'s INC6 section): the manifest's 27
`[[ui.panel.widgets]]` TYPED into `InstrumentDisplay.widgets` (18 enum_select / 5 toggles /
3 sliders / 1 status label; param ids + D7 gates resolved to indices at parse; an unresolvable
or unseen-kind widget DROPS — the host paints no control it cannot route); the band paints at
Full LOD (enum chips open §8.4's EXISTING picker, toggles flip through the param door, sliders
ride the param-row idiom — every control an undoable `SetParam`, the live ledger hears it);
hidden cells are not painted, not touchable, not registered; the at-rest card says in words
"params edit; the wall re-renders live when the loader runs" (never a frozen lie); the audit
registers every routed cell (`canvas/panel/{node}/{widget}`, class S = D16's given 44-px floor;
the declared M rides along as data — an M-sized band would be a D6 movement, flagged for S5,
NOT decided). **Defect #100 found + fixed + ledger-rowed**: manifest-parsed enums carried a
zeroed range, so every picker selection clamped to "no change" (the discovery door never caught
up with §8.4); an enum's domain is now its option-index range, regression-pinned on the shipped
manifest. S2 gates: workspace **1054/0** (+7), audit **PASS (0 failures, 75 rows**, was 71),
all other gates unchanged-green; two more failable injections (the D7 gate forced open → 3
FAILs; #100's `max: 0.0` restored → 4 FAILs), reverted byte-identical. S3 landed on top (full
record in `WO020-STATE.md`'s INC6 section): `sparq-streams/src/store.rs` — the key/cadence
overrides store in the user-data dir (`sparq/streams.d/overrides.toml`, D18's location; the
`SPARQ_STREAMS_OVERRIDES` env door for tests): the 10 s floor REFUSES IN WORDS and keeps the old
value (O-3, at the setter AND at file load), env WINS over the file (O-4 — the composed closure
feeds the fetch's existing `{KEY}` redaction unchanged), the key VALUE has no reading door (the
mask `••••last4` + the state words `SET (env)`/`SET (file)`/`UNSET`/`KEY NEEDED`/`—` only; a
hand-implemented masked `Debug`), a malformed file is an empty store with words, the save is
tmp+rename+fsync, and file keys resolve per ENV VAR (one NASA key typed on any sibling row
serves all three — the closure's rule, every sibling row shows it). The broker reads
override-then-registry (`due` + the stale horizon follows via `Window::set_cadence`); the CLI's
EVERY door composes through the store (list shows the effective cadence, the state words and
the store's path); the dock's STREAMS tab is ALIVE under the `streams` feature (O-2): LED +
word statuses (fixture-fed under an **`AT REST — fixtures`** header — never a frozen lie, D20),
the age in words, the cadence field (override bright, registry dimmed beside it), the KEY field
masked — and DISABLED WITH ITS WORDS when the env governs (the precedence is visible; a tap
still refuses in words, the menu-row convention), the rows scroll under the dock's own pan, the
fields are 44-px class-S registered touch targets, and the typing is the rename entry's
input-event idiom (Enter commits through the store's doors, Escape cancels in words, the
cadence pre-fill is REPLACED on first type, the key echo is bullets). S3 gates: workspace
**1055/0**; streams(+net) **106/0/1-ignored** (+10); `-p sparq-app --features ui,streams`
**47/1** (the known env red only); `ui --audit` **PASS in BOTH faces** (`ui` 75 rows;
`ui,streams` **80 rows** — smoke 58's five checks: the tab alive + the honest header, the 5 s
refusal, the honoured 10 s override (file written + reloaded), the MASKED key persistence (the
sentinel grepped out of every log line and frame text — it rides the user-data file alone), the
env-wins disabled field); clippy green on all five cells incl. `ui,streams`; fmt clean; the
python gates and the observatory untouched-green. Three more failable injections (the floor
deaf, the key leaked to the log, the precedence inverted) — each caught, each reverted
byte-identical. The acceptance's DEVICE half (the NASA key flipping neo/epic/power from
429/DEMO_KEY to the key's rate) needs the live driver — S4's slice and the run sheet's row,
exactly as the plan scoped it; the sandbox proved the mechanism behind it (mock transport +
temp-dir store). Bundles: `handoff/sparq-wo020-inc6s1/s2/s3.bundle` (+ sha256s, prerequisite
`b2445d6`; the S3 bundle carries the whole INC6 branch — S1's recovery was proved by a
fresh-clone fetch + ff-merge this session). S4 landed on top (full record in `WO020-STATE.md`'s
INC6 section): the D19 token `canvas.instrument_display_hz = 15` ruled into `layout.toml`
(additive, regenerated, the bundle round-trip green, no new token_audit violation);
`ui/live_display.rs` — the `LiveDisplay` seam, the pacer (66 ms period; edits/LOD force the
next frame; FIVE CONSECUTIVE overruns bypass the DISPLAY instance with §3's words, an edit
re-arms — the audio instance untouched, decision D's isolation), the live-surface shelf and the
band switch (LIVE where the instance carries it, AT REST otherwise — the meters' rule
extended); the shell's per-frame tick (the launch shelf attaches to the module's first node,
the edit diff reads the GRAPH so no future door can forget the force); `ui/streams_driver.rs` —
the driver whose `poll_once` IS the loop body (hermetic, mock-proven: the store key reaches the
request URL, the 10 s override paces, the words never carry the key) with the thin device
thread (a control-plane condvar wait, never the banned sleep; `wake` on edits; `Drop` stops and
joins); the provider swap (the tab/label read the LIVE broker mirror first — `LIVE —` header —
the fixtures second, `AT REST — fixtures`), `start_live_streams()` an EXPLICIT window-host door
(never `new()`: the headless audit stays hermetic even in a streams-net build); and
`ui/instrument_launch.rs` — **DEVICE-FIRST-COMPILE** (the §8 wall): the D17a launch
registration (load_package → the display half parks on the shelf as a `RuntimeDisplay`
(stages.rs's own budget_call/call_draw idiom, D13's interchange into the at-rest parser) and
the audio half registers through the registry's factory door — the factory RELOADS per
instantiation, a `LoadRefused` Failed-module covers the exception; PLAY's #58 lifts
STRUCTURALLY), desk-checked call by call against the vendored API, with **`scripts/test008.bat`
as its run sheet** (launch words → PLAY → the living wall → the watchdog injection on a
max_fuel=100000 copy → the STREAMS tab LIVE + the NASA-key flip = S3's device half + the
on-device redaction findstr; DO NOT CANCEL rides in its header). S4 gates: workspace
**1061/0**; `-p sparq-app --features ui,streams` **56/1** (the known env red only; carries the
driver's 3 hermetic tests + the seam's 7); streams(+net) **106/0/1-ignored**; `ui --audit`
**PASS in both faces** (`ui` 79 rows; `ui,streams` **84 rows** — smoke 59's four: the attach +
the 15 Hz pace, the band's actual px + a resize reaching the guest, the edit force, the
five-overrun bypass); clippy green on SIX cells (incl. `ui-window,streams,streams-net`); the
python gates green — `check_text_io` caught test008.bat's unescaped echo parens (defect #69's
gate earning its keep; fixed); observatory 124/0 untouched. Three more injections (the pacer
always-due, the bypass threshold 5→50, the driver's env door skipping the store) — each
caught, each reverted byte-identical. A near-miss is recorded in STATE's S4 judgment calls
(a scratch file's `rm -rf` took the checked-in `probe_alloc.rs` with it — caught by
`git status`, restored byte-exact, zero trace). **The S4 acceptance rows [B]/[D]/[E] are
DEVICE-PENDING and ride the next run-sheet round with session 8's outstanding gates.bat re-run
and [G]/[I] eyes.** S5 landed on top (full record in `WO020-STATE.md`'s INC6 section): the host
RATE row — 40 px under the guest's display on the `out/main` info band's own token and
precedent (D16: host words in the card, never guest widgets), reserved by the LAYOUT on every
instrument card in every build (the geometry is never feature-dependent; a build without the
stream plane shows honest words, never a pretend field), the guest's band stopping where the
row starts. The field is D20's SECOND DOOR to the one store: the tap routes `Hit::RateField` →
`CanvasEvent::RateField` (the RenderWav division — the canvas routes, the shell acts), the
entry commits through `Overrides::set_cadence` (the O-3 floor refuses in words, the save + the
driver's re-pace ride along), and the row's WORDS NAME THE GOVERNED STREAM + the shared-feed
truth (`RATE · CELL 07 → space.epic: 3600 s — 2 panel(s) ride this feed and share its rate`),
resolved from the node's LIVE params per frame — moving the CELL dropdown moves the governed
stream THE SAME FRAME (smoke-proved). The field registers in the audit with the card rows'
dense-exception idiom. Card numbers moved, BAND numbers did not: chrome 168→208, default card
**1120×768**, maximum **2208×1328**; S1's fit row re-cut to O-1's OWN words (the BAND fits the
1624×740 rect at zoom 1; FOCUS frames the CARD at 0.90, Full LOD; the 16-cell wall reads
126 px at FOCUS, 62 px at the ruled floor). The convergence sheet was RE-SHOT at Full LOD
(`ui --svg-out … --review`, sha `56e446d3…`, 173 084 B — the half-face card, the typed band's
chips and enum readings, the RATE row, the fixtures' `LIVE 16/16` sentence), recorded by sha +
content grep and NOT checked into `design/mockups/` (token_audit scans `*.svg` there — a
shell-dumped SVG would move the standing-red baseline; the wo012 precedent is a PNG, and the
PNG re-shoot WITH THE LIVE WALL is the device round's). The mockup audit re-run: the IDENTICAL
standing set (14 groups/298, zero new). S5 gates: workspace **1062/0**; app ui,streams
**56/1** (the known env red only); `ui --audit` **PASS both faces** (`ui` 80 rows;
`ui,streams` **88 rows** — the RATE-geometry row + smoke 60's three checks); clippy green on
all six cells; fmt clean; python gates green; observatory 124/0 untouched. Three more
injections (K1 the words unnamed, K2 the resolution dead → all three smoke-60 checks, K3 the
card formula drops the row) — each caught, each reverted byte-identical; one transient 3-FAIL
run mid-sequence was traced to the pre-hardening smoke's FOCUS framing dependency and the smoke
is now deterministic by construction (explicit camera at the dense floor's exact edge, measured
Full-LOD precondition) — the event and the remedy are recorded in STATE's S5 judgment calls.
S2's touch-class flag is CARRIED TO THE OPERATOR (the manifest declares M; the band gives D16's
44; an M-sized band is a D6 movement — not decided here). Bundles:
`handoff/sparq-wo020-inc6s1..s5.bundle` (+ sha256s, prerequisite `b2445d6`; the S5 bundle
carries the whole INC6 branch). **Next: the DEVICE round — `scripts/test007.bat`'s
uninterrupted gates re-run (session 8's [K]) + `scripts/test008.bat` (the S4 launch words,
PLAY #58 gone, the living wall, the watchdog injection, the STREAMS tab LIVE + the NASA-key
flip = S3's device half, the redaction findstr) + the [G]/[I] eyes + the convergence PNG
re-shoot — or operator rulings on the carried flags.**

**Previous update:** 2026-10-06, continuation session (same sandbox) — **WO-020 INC3 CLOSED: the
Observatory's guest source project, the five-file package, and the wasm component, built and
measured in the sandbox**. Operator scope: "continue with wo20 build" → the next open increment per
`WO020-STATE.md` (INC1+INC2 already closed). Landed (full record in `WO020-STATE.md`):
`tools/make_coastline.py` + `instruments-src/observatory/assets/coastline-110m.bin` (Natural Earth
110m → Visvalingam-Whyatt **3 500 points / 14 524 B**, inside D12's ≤3.5 k/≤160 KB, `--check`
gated); the source project `instruments-src/observatory/` as **its own cargo workspace**
(core/wasm/harness; the guest SDK `exclude`d — path-dep auto-membership would otherwise let
`cargo fmt --all` reformat the frozen contract crate, the root's exclude precedent);
`gen_manifest.py` generating the 16 × 24-option enum walls + 26 params + §5.2 toolbar + 16 `param:`
source bindings from `streams.toml` (**closing both §10 draft gaps INC2 flagged**: history declared
in **seconds** — UNITS is closed and has no `min` — and bool defaults as **floats**), plus the
guest-side `streams_table.rs` registry mirror, both `--check`-gated; `observatory-core` — pure,
zero-dep: the display-list IR (field-for-field display.wit mirror), the frozen data-record mirror,
§5.1 layout maths as tested functions (**4×4 height erratum found by building: the §5.1 table prints
270, the plan's own gap=16 gives 268 — the formula wins, recorded**), the 26-param decode, a
deterministic params-only state blob, **the seven §5.5 renderers** + §5.3 cell chrome + §5.5 ticker
band + §5.7 LOD ladder + §5.8 budget counter; `observatory-wasm` — the SDK glue (port-less `process`
no-op mirroring params, `draw` = `sources.snapshot` → core, state = params); `observatory-harness` —
fixtures → core → **display-list JSON interchange + SVG at the three breakpoints** (its own
token-resolving SVG painter = the sandbox stand-in for INC4's `displaylist.rs`), reviewed visually
against the display-sheet idiom; **the component BUILDS in the ~1 GB sandbox** (risk §12.2
discharged): `wasm32-unknown-unknown` + `wasm-tools component new` (v1.261.0 prebuilt) → **valid
206 KB component** exporting `sparq:instrument/guest@1.1.0` (release `strip = "debuginfo"` keeps the
name section for device traps; 2.9 MB → 206 KB); `tools/observatory_package.py` assembling
`instruments/observatory/` (**4 files ≤ the 5-cap**; `preview.svg` absent BY DESIGN — stage 6
generates it; the tool refuses stale/incomplete inputs); **`sparq mod validate
instruments/observatory/` measured: stage 1 schema PASS (the full 16-cell manifest, every stream
binding resolved, port-less legal, `network = "none"` honoured), stage 2 package PASS, stages 3/4/6
REFUSED IN WORDS, 5 static half, 7 unsigned→badged; GATE PARTIAL, exit 1 by design**; `sparq mod
list` discovers it **loadable**. The D5 port-less proof as tests (+9): kernel graph + registration +
executor (a port-less-ONLY graph builds; the wall coexists with the audio path and the set still
plays) + `bridge::build` + `browser_catalog` (instrument layer, 26 params) + canvas layout (nothing
refuses; **recorded for INC4: the anatomy's `rows = max(1)` still draws one cosmetic port row on a
port-less card — the D6 zero-port-band sizing is INC4's**). Additive token change:
`layout.marker.radius_s/m/l = 3/5/8` (the `points-item.size` class §5.5's "size class by magnitude"
needs) — regenerated, `token_gen --check` deterministic, bundle-pin test green, no new token_audit
violation. Hygiene first: **the CI `check_text_io` gate was RED on main** (INC1's
`streams_record.py`: bare pathlib writes + no UTF-8 reconfigure) — fixed in the first commit of this
session. Frozen-surface frictions recorded, not papered over (ADR-010 review-trigger material):
`trace-item` carries no box → per-cell series ride polylines; `rect-item` carries no colormap
channel → Kp bars encode level as height + the storm line; `glyph-run` carries no anchor/width → the
guest estimates advance for right-align/truncation; one wall-wide `history` param vs daily feeds
(POWER shows ~1 point at the 3 h default) — per-cell history is a later addition; AQI/Kp threshold
bands as registry data rows (LATER.md). **Gates (measured):** root `cargo test --workspace`
**991/0/1-ignored** (+9 over INC2's 982); instruments-src workspace **124 lib/unit + 6 golden/drift**
tests (golden IR sha256 pinned `d4a03d25…`; streams_table↔registry, Metrics↔bundle,
manifest-defaults↔core drift gates); clippy `-D warnings` CLEAN in BOTH workspaces; fmt CLEAN
(per-package in instruments-src — `--all` there would adopt the SDK); python gates token_gen /
unsafe_audit / module_docs / check_text_io / sync_check PASS + `make_coastline --check` +
`gen_manifest --check` PASS (token_audit still fails the pre-existing §18 mockup SVGs — no INC3 file
flagged; that gate's owner is the mockup work); the WO-005 golden hash `ba577186c988db21` is
unchanged; WIT pins, the 24 module manifests and `SYNC-STAMP.txt` UNTOUCHED. Committed in four
slices (`cef2385` hygiene, `48a56c2` core+generator, `0bd7f43` glue+harness+gates, `7c2f4a7`
package+D5) + this seal; bundled `handoff/sparq-wo020-inc3.bundle`.

### WO-020 INC4 — CLOSED 2026-10-06 (host painters, picker, FOCUS, D6, at-rest wall, render CLI)

Slices on `wo020-observatory`: `33e81b9` (A: sparq-ui displaylist core + SVG back end + zero-dep
JSON + token LUTs from token_gen), `ab5167f` (B: `ManifestSources`/`ReplayProvider`/`status_label`
under the `streams` feature), C1 (D6 sizing: `NodeSpec.display`, `node_size` instrument branch,
`Well::Display`, browser `sections()`), `c505903` (C2: `ParamSpec.options` in the contract, the
picker + FOCUS toolkit-independent, `ParamDesc::settable`), `8ada49e` (C3: at-rest store, egui
back end, review sheet, browser headers, toolbar FOCUS, §5.2 label, `instrument render`), gate
hygiene, + this seal; bundled `handoff/sparq-wo020-inc4.bundle`.

Measured at closeout (no number written until measured):

* **Coverage row:** the wall card is **2208×1288**; the Design chrome's canvas rect at 2560×1600 is
  **2264×1260** ⇒ **97.5 %** (audit rule ≥ 95 %, `ui --audit` row PASSes; the 1288>1260 note records
  FIT ≈ 0.98 per D6's own arithmetic). Port band on an instrument card: **zero** (the anatomy's
  cosmetic `rows = max(1)` row no longer applies to instrument-layer cards).
* **Picker:** rows exactly **44 px** (the touch floor token); ≤ 8 visible rows then scroll; a
  viewport too small for one row gets WORDS. Round-4 **OWED item 11 discharged**: `enum` params are
  EDITABLE now — card row or inspector row opens the sheet, a row tap is an ordinary undoable
  `Op::SetParam` (the option index), outside closes, wheel scrolls.
* **FOCUS (§8.5):** three doors (node-menu row, header double-tap, toolbar button) → one
  `CanvasState::focus_node` = the existing `zoom_to_fit` with bounds = one card.
* **Painters:** one resolution, two back ends; the egui face chorded curves and decomposed dashes
  (egui has neither); unresolved token ids skip with a diagnostic (fail-soft) and the count is said
  in the band's corner. `sparq instrument render` reports the §5.8 budget vs the manifest's declared
  `gpu_class`: **22 343 vertices / 497 instances / 16 264 heat cells vs medium — fits**.
* **At-rest:** `$SPARQ_ATREST` / cache-dir `dat_observatory.ir.json`; the harness `--atrest`
  publishes it; a malformed file logs words, a missing one paints WORDS in the band. The
  interchange gained `display_w`/`display_h`; **golden IR re-pinned `bdf59cdb…`** with the reason at
  the pin (format change, not a render change).
* **Browser/dock:** instruments group under their own header, first; header rows are dim titles,
  a tap refuses in words, a query drops headers (a search is a flat answer).
* **§5.2 status label:** host words in the instrument panel band, per frame, from `ReplayProvider`
  (fixtures here; `BrokerProvider` on device is INC5's door, same trait).
* **Gates:** root **1033 passed / 0 failed / 1 ignored**; clippy cells workspace + `ui` +
  `ui,streams` + `ui-window` + `bootstrap-audio` all 0 warnings (the alsa cell needed
  `pkg-config`+`libasound2-dev` in the sandbox); fmt clean both workspaces; instruments-src
  **124 passed** + 6 golden/drift, clippy 0; python gates PASS; `token_audit` back to the
  pre-existing 14 groups / 298 occurrences (§18 mockups only — one new R6 hex in a sparq-ui test
  was caught and fixed the same pass); WO-005 golden hash `ba577186c988db21` unchanged; WIT pins,
  the 24 manifests and SYNC-STAMP untouched; `ui --audit` PASS with four new instrument rows.
* **Convergence sheet:** `reference/observatory/convergence-wo020-inc4.png` + its README (the
  regeneration recipe; the 5 MB SVG stays out of git). The sheet shows the library grouping, the
  toolbar FOCUS, the at-rest wall on canvas, and — in the log — the honest #58 refusal that names
  INC5's loader as the door to live playback.

**Delivery (same day, handoff prep):** INC1–INC4 packed for the device —
`sparq-update-2026-10-06-wo020-inc4.zip` (FULL tree, **471 entries**, stamp NOT riding per the
rev-3/r8 rule; sha256 in `handoff/sha256sums-wo020.txt` + the delivery message), the operator's
`WO020-INC4-RUN-SHEET.md` (apply → re-stamp → gates → the WO-020 device column → send-back; both
honest refusals stated: validate's stages 3/4/6 and PLAY #58 until INC5), `SYNC.md`'s new top
entry (the 90-path namelist vs `96fe57d`), the self-verifying packer `handoff/make_pack_wo020.py`
(entry-by-entry zip↔tree hashes + the count/MUST-NOT-MOVE assertions — the pre-pack gate LATER
idealized, discharged), and the closing `handoff/sparq-wo020-delivery.bundle` (`96fe57d`..HEAD,
sha sidecar). Two frictions recorded for INC5, not papered over: the Windows at-rest cache-dir
disagreement (harness `%USERPROFILE%\.cache\…` vs app `%APPDATA%\…` — the run sheet uses the
`SPARQ_ATREST` env door) and the untracked guest `Cargo.lock` (device component rebuilds resolve
unpinned; the shipped component is byte-pinned).

**Next: WO-020 INC5** (device-only: wasmtime loader + package registration so PLAY stops refusing,
`BrokerProvider`, stage-6 at-rest publishing, the full validate chain, live acceptance).

**Previous update:** 2026-10-06, INC1+INC2 session (earlier the same day) — **WO-020 INC1 +
INC2 CLOSED: the stream broker's hermetic half AND the contract additions, built and measured
green**. The operator scoped the session (question tool) to **INC1, then INC2** and
**install-Rust-and-verify** (not source-only).
Environment from nothing — rustup via python (no curl/wget in the box) + `apt-get install gcc` for
the linker rustc needs; neither persists across snapshots, so the resume recipe re-installs both.
Pristine baseline measured BEFORE any edit (**925/0/1-ignored** = the round-8 seal exactly), then
built the first increment of The Observatory's stream plane (ADR-011): the new crate
**`sparq-streams`** (the 23-stream `streams.toml` registry as the single source of truth + the §3.3
dead ends as comments; the frozen `data-record` mirror; the five `observatory/*@1` schema builders;
the registry with KEY-NEEDED-in-words, D14; clock-free rolling windows + the stale(3×)/offline(10×)
policy; dependency-free timestamp parsing; **all 23 normalizers written against RECORDED payloads**;
the disk cache; fixture replay), **`tools/streams_record.py`** (reads the registry, fills the
endpoint templates, records raw payloads + `_meta.json` + `PROBE-LOG-2026-10-06.md`), the recorded
fixtures (**23/23 at HTTP 200**, FIRMS synthetic — free key unavailable; each fixture byte-faithful,
worst ratio **1.000** ≤ 1.2), and the app CLI **`sparq streams list|cache`** (hermetic) +
probe/fetch/tail/record **refusing in words, exit non-zero** without `streams-net` (the transport
half is the device increment INC5). **Gates (measured, not invented):** `cargo test --workspace`
**971/0/1-ignored** (+46), `sparq-streams --features streams` **75/0** (incl. 23 per-stream
normalizer tests + the registry↔normalizer drift gate + bit-identical-twice determinism),
`sparq-app --features streams` green, clippy `-D warnings` **CLEAN** (default + both feature cells),
fmt **CLEAN**, and the **default build stays zero third-party deps** (`cargo tree` verified;
serde_json only under `streams`). WIT pins, goldens, the 24 module manifests and `SYNC-STAMP.txt`
UNTOUCHED. **API drift caught by recording live (the probe-first rule, §12.6):** NASA POWER now
**422s on dashed dates** (wants compact `YYYYMMDD` — fix landed: `{D-N}` placeholders alongside
`{d-N}`) and EONET's `categories=wildfires` is **ignored under `limit`** (returns the all-events
superset — flagged in `streams.toml` for INC3/operator, not papered over). **INC2 then added the one
contract addition the stream plane needs — additive, WIT UNTOUCHED (§7.4 proved: `wit_snapshot`
byte-identical, no git diff under `docs/api/instrument-wit/`):** `docs/adr/011-stream-plane.md`;
`CodeKind::StreamUnknown` = **`E-STREAM-UNKNOWN`** (the catalogue's 26th, count test moved
in-commit); the `ui.displays[].sources[].stream` binding (`decode.rs` shape-checks `port`⊕`stream` +
the `param:<id>` form; `validate.rs` stage 1 resolves ids against the registry and the `param:`
enum's options, + `visible_if.param` existence — plan D4/D7); `visible_if` specified as
`{param, equals}`; the `node_width_instrument_max = 2400` layout token + card-sizing rule (D6,
`token_gen` re-run, `--check` deterministic); and the schema-doc / field-table / MODULE-BUILD-GUIDE
§5 rows. **9 new validator tests** (`crates/sparq-host-wasm/tests/stream_bindings.rs`): the
representative Observatory draft passes stages 1–2 with real bindings, each hostile variant fails
its rule in words, and all 23 registry ids are accepted as fixed bindings (the registry↔contract
drift gate). **INC2 gates (measured):** `cargo test --workspace` **982/0/1-ignored** (+11 over INC1),
`sparq-streams --features streams` 75/0, clippy CLEAN (default + host-wasm + streams cell), fmt
CLEAN, `token_gen --check` CLEAN, `api_snapshot` + the three field-table drift tests PASS, goldens +
the 24 manifests untouched. Two §10 draft-manifest gaps found by building it (INC3 generator
concerns, recorded not papered over): `unit = "min"` is not in the closed `UNITS` vocabulary, and
bool params need `default = 1.0` not `true`. Committed `7da006a` (INC1) + seal + the INC2 commit;
bundled `handoff/sparq-wo020-inc1.bundle` + an INC2 bundle. **Environment note:** the snapshot
dropped the toolchain AND `.git` between turns — recovered by re-installing rustup+gcc and
re-cloning origin (base `48d3074`) then fetching the INC1 bundle; exactly the loss class the
per-increment bundle discipline exists for. No operator rulings were needed beyond the two that
scoped the session. **Next: WO-020 INC3** (the guest source project `instruments-src/observatory/`,
the seven renderers, `make_coastline.py`, `gen_manifest.py` — which must emit valid units/defaults —
the five-file package + the wasm build attempt), then INC4 (sandbox painters/shell) and INC5
(device). Full record: `WO020-STATE.md`.

**Previous update:** 2026-10-05, END of sixteenth session (Linux sandbox box) — the **WO-018
REBUILD round: the zero-dependency half BUILT and measured green**. The session opened on a
fresh clone of the re-publicised origin (`6681f87`) and confirmed the loss with evidence: the
round-7 PROSE (`ROUND7-HANDOFF.md`, `WO018-STATE.md`) had reached origin, but the round-7 CODE
had not — the uncommitted WO-018 tree died with its sandbox (ROUND5 §3.1's defect class; no
bundle carries it: round 7 made zero commits by design). Rebuilt from the handoff + state card +
ticket + the frozen contract on the round-4 precedent (`READ-ME-FIRST.txt`), keeping all seven
recorded ROUND7 judgement calls, then did what round 7 never could — **compile and gate it**:
environment from nothing (rust 1.99 standalone + zigcc-direct + the MSVC std face), pristine
baseline measured BEFORE any edit (**909/0/1-ignored** = the round-6 seal, exactly), then the
full inventory rebuilt: `sparq-host-wasm` (the five-file package model + the
`E-PACKAGE-FILECOUNT` cap, launch-vs-strict profiles, sorted discovery + loose-file words, the
`gpu_class` ceiling table = **freeze debt #1 CLOSED as data**, the §7 gate's static stages with
the runtime stages REFUSED IN WORDS — the PARTIAL verdict), `sparq mod validate` (PARTIAL exits
non-zero: a partial gate is not a hand-in) / `sparq mod list [--root] [--strict]` (the CI
smoke), the catalogue **25→30** (five v1.1 codes pre-registered; `E-PACKAGE-FILECOUNT` wired),
`check_gpu_class` at decode + the `GPU_CLASSES` drift/value pins + the field-table domain row,
`instruments/` + its README, `docs/instrument-host.md` (loader architecture, fuel/epoch↔watchdog,
the **two-instance ordering = freeze debt #2 CLOSED as spec**, the code-wiring matrix, the
runtime test plan, §8's blockage record + retry recipe), the CI cells (feature-on Linux + MSVC
crosscheck — both recorded NEVER-MEASURED with the narrowed risk + documented fallback).
**Gates (measured):** `cargo test --workspace` **925/0/1-ignored** (+16 = the state card's
itemised expectation exactly), fmt + clippy `-D warnings` CLEAN (workspace, all targets), **all
default-feature MSVC crosscheck cells CLEAN** (module-api, host-wasm, music, kernel, audio, ui,
app), python gates clean, `module_docs` **24/24**, `mod list --strict` exit 0, WIT pins
UNTOUCHED and passing (the rebuild touched no frozen surface), `sync_check` FAIL-BY-DESIGN
(15 files; the device re-stamps). **The wasmtime probe, measured:** the dependency tree checks
green to `cranelift-codegen`, then OOM SIGKILL at 4 m 21 s — the blockage is this sandbox's
1.06 GB categorically (swap refused, EPERM; threads healthy throughout), NOT the pin, NOT the
code: the runtime half is deferred to a ≥ 2 GB host (device/CI) with the recipe in
`docs/instrument-host.md` §8. Committed in the recipe's four increments (`450c38b`, `7eee56b`,
`b3f2091`, `51b25aa`), bundled after each; no operator rulings were needed (the eight NEW
judgement calls are flagged in `ROUND8-HANDOFF.md` §4). Round-4 device obligations stand
untouched. **Next: the WO-018 runtime increment** (`docs/instrument-host.md` §2–§4 + §7's test
plan, on a ≥ 2 GB host), then WO-019. Full record: `ROUND8-HANDOFF.md`.

**Previous update:** 2026-10-05, END of fourteenth session (Linux sandbox box) — the **WO-017
CLOSE round: contract v1.1 FROZEN**. Fresh clone of the round-5 tree from the re-publicised
origin, environment rebuilt from nothing (ROUND5-HANDOFF §7), baseline measured green BEFORE any
edit (899/0/1-ignored, python gates clean). **Four operator rulings** (question tool,
2026-10-05): `data-value`/`data-record` **ratified as-is** (recheck trigger = the native
data-port landing, Phase 5; until the host routes data, an instrument declaring a `data` port is
refused at load in words) · **`t-wall-ns` REMOVED** from `host.time-info` (guests never read a
wall clock — draw's animation clock is `frame-context.time-sec`, data records are host-stamped;
`now()` is fully deterministic) · browser layer work **data-side only** this round (visual
grouping + badging chrome = WO-018; the sandbox cannot run `ui --audit`) · **freeze stamped**.
What landed: native `classification.layer` (typed `Layer`, default backbone — the 24 first-party
manifests re-validate with ZERO edits; `layer = instrument` requires `tier = t2` as
**`E-LAYER-MISMATCH`**; error catalogue 24→25) + `ui.displays[]` content-checked against the
frozen §9 shape (kind/lod vocabularies, unique ids, `min_size`, colormap prefix,
`sources[]{id, port}` — the binding shape was specified nowhere and is now frozen in
manifest-schema §9 + the field table with a drift pin) + the **template acceptance criterion
made mechanical** (`include_str!` of the checked-in skeleton manifest, decoded in a test) +
`BrowserItem` carries `Layer` audit-neutrally (pinned by test) + **token bundle v1 generated**
(`tools/token_gen.py` → `design/tokens/generated/token-bundle.json`; double-gated: `--check`'s
round-trip invariant + `tests/token_bundle.rs`'s sha256/semver pins — one-commit propagation or
CI fails) + **the WIT hash-pinned** (`tests/wit_snapshot.rs`: nine files + the directory set;
every new gate proven failable by deliberate corruption) + **SDK at freeze** (vendored `wit/`
snapshot + drift test; crate 0.1.0→**1.1.0**, carrying the contract version) + module-api §11's
`instruments/` discovery slot + wasmtime pin recorded (**49.0.2**, Component Model + WASI 0.2) +
the fm-terrain paper walkthrough (two guide amendments logged: data-port status, sources shape).
**Round-trip re-proved against the EDITED contract**: both wasm faces rebuilt, `wasm-tools
component new` (**recipe correction: `jco componentize` is JS-only — round 5's §7 note was
wrong**), `jco transpile`, smoke **11/11**. Gates on this box: fmt/clippy CLEAN,
`cargo test --workspace` **909/0/1-ignored** (+10 new tests — the digest expectation MOVES: src
fp now **96f/2717264B**), five python gates clean, `module_docs` 24/24, WIT parse-clean
(jco 1.35), SDK tests 3+1; `ui --audit` NOT re-run (no UI behaviour touched — the one
UI-adjacent change is a data field; the device re-runs it in gates.bat). `sync_check` FAILS BY
DESIGN (11 stamped files moved; the device re-stamps). WO-017 acceptance: **4/4 ticked** (§3b).
Open BY DESIGN: the `gpu_class` ceiling table + two-instance `configure` ordering (WO-018 debts,
ruled not contract gaps) and the five remaining validation codes' wiring (WO-018 validator).
Round-4 device obligations stand untouched (test006 A–U, test004 attempt 4, defect #95, the
dropdown picker). **Next: WO-018** — loader/validator design notes + the host-side half of the
round-trip test. Full record: `ROUND6-HANDOFF.md`.

**Previous update:** 2026-10-04, END of thirteenth session (Linux sandbox box) — the
**INSTRUMENT-LAYER PLANNING ROUND** (no engine code): the operator ruling of 2026-10-03 makes
the library two layers — backbone modules (first-party, T1) and **instruments** (the
performance/control tier: complex, visually rich, third-party, **T2 WASM sandbox pulled forward
from Phase 5**, host-rendered displays styled by token ids so design changes re-theme
instruments with zero intervention, ≤5-file `.sparqmod` packages dropped in `instruments/`).
Recorded as **ADR-010 / D-14**; written: ADR-010 + ADR-002 amendment, SPARQ-PLAN (§6.9 + six
more sections), root `MODULE-BUILD-GUIDE.md` v1, WO-017…019 pre-ticketed (§3b — Phase 0's exit
gate untouched), manifest-schema `classification.layer` + six error codes,
`docs/api/instrument-wit/` (**`sparq:instrument@1.1.0` skeleton v0.2**, aligned field-for-field
against `sparq-module-api`, parse-clean via wit-parser), `tools/sparq-module-guest/` (**SDK v0.1
skeleton** + noop-instrument example — builds to wasm; jco componentize + JS smoke **11/11
PASS**), `reference/instrument-template/`. **The parallel-build ruling: YES behind the WO-017
freeze set** (ADR-010 decision 7). Gates re-measured on the sandbox box: fmt/clippy CLEAN,
`cargo test --workspace` **899/0/1-ignored** (digest unchanged), five python gates clean,
`module_docs --check` **24/24**; `ui --audit` NOT re-run in-sandbox (ui-window stack absent;
zero UI code touched — device re-runs it in `gates.bat`). **TWO INCIDENTS, read
`ROUND5-HANDOFF.md` §3 before anything else:** (1) the sandbox lost its `.git` objects AND
origin went private — local history rebuilt as snapshot commit `9809593`; delivery is the
full-tree overlay **`sparq-update-2026-10-04.zip`** (+ a review bundle), device stays canonical;
(2) defect #94's class struck again — `modules/out/` stripped by the snapshot layer,
`out/main/sparqmod.toml` **reconstructed** from its generated doc (machine-verified 24/24;
comments differ). `sync_check` **FAILS BY DESIGN** on this tree (Cargo.toml's exclude line +
the reconstruction; Cargo.lock excluded from the pack on purpose — zero new root deps): the
device re-stamps after applying. Round-4 obligations stand untouched: test006 A–U, test004
attempt 4, defect #95, the dropdown picker. **Freeze checklist open items: WIT README §5 /
handoff §6.**

**Twelfth session, round 4 (2026-10-02, Linux sandbox) — operator UI round 4: the cable-node /
living-wells round, REBUILT from `ROUND4-HANDOFF.md` after the loss (defect #94), GREEN and
SEALED (`sparq-update-2026-10-02.zip` rev 4, full-tree).** What landed, deepest first — the full
item-by-item table is in `UI-CHANGES-PLAN.md`'s round-4 sheet, the session record in
`ROUND4-HANDOFF.md` §0–§0e: **the executor adopts** (D1: `adopt_runtime` — an unchanged node
keeps its state and its allocations across a hotswap; the kernel boundary hook is
`FnOnce(&mut T, &mut T)`; the command-ring fast path stays declared LATER behind the bridge's
`TrimGainMap` door). **Cable nodes end to end** (D15: `WireTrim`/`Op::SetTrim` in the model, hover
ghost → tap-insert at identity → drag axes → tap-remove in the gestures, the trim node painted,
and the bridge's voice — audio → invisible synthesised `util/gain`, cv → `set_cv_trim`, control →
param-mod args, mult-collapse composed affinely, event/data/spatial refused IN WORDS; identity
renders BIT-IDENTICAL, pinned by audit smoke 54 through the real gesture door; vocabulary in
`gestures.md` §4f). **The wells came alive**: the clock's four division rings (wrap-correct
`phase`), the seq's walking lights, the rand card's step bars (`step_hash` pinned in BOTH crates
— the seed-42 ring reproduces exactly), the quantizer's 12-key keyboard (membership fill,
passing-pitch light, Custom-mode tap with the worded preset refusal), the rms card's 240-frame
rolling graph, the clip LED latched on the master's meters, the svf's response curve moving under
its cutoff-mod cv, the mult strip at quarter width with its words off. **Three new modules** —
`fx/fold`, `util/quant` (15 scales + Custom mask), `mod/rand` — plus `clk phase`, `seq step`,
`rms slew`, `delay sync`: the first-party set is **24**, `module_docs` 24/24. **Tokens moved by
ruling**: event cables SOLID (D9 — the control-wire dash stays, a different encoding;
mockup-review finding 25) and ports float 6 px (D10, finding 26). Gates, all on this box:
`cargo fmt --all --check` CLEAN · clippy workspace + `ui` + `bootstrap-audio` cells 0 diagnostics
· **`cargo test --workspace` 899 / 0 / 1 ignored** (+67 over rev 3's 832) · `cargo build
--release` · selftest **9/9, `ba577186c988db21`** — the DSP goldens never moved · the three
pinned exec renders EXACT (`mod-demo` 1 s `1621e1f65b1b64e1`, `drum-demo` 2 s `f2303f13aa0cf299`,
`demo` 2.8 s `53de3b1f3f40e3c9`) · **`ui --audit` PASS, 0 failures, 59 [PASS] lines** (four new
round-4 smokes: the clip latch, the phase pipeline end-to-end `19456/24000`, the encoding pin,
the cable-node render hashes) · `modules --strict` 24/24 · every python gate · sync re-stamped
**`sparq-round4-2026-10-02`, 162 files, `src 96f/2694477B`**, `sync_check` OK.
**Defect #94 (the loss, recorded):** the between-turn sandbox reset dropped every `out` path
component (defect #93's trap) AND the repo had gone private — round-4's code existed nowhere.
Remedied by this rebuild and by the full-tree pack. **The trap re-struck THIS tree at the S9
wrap-up boundary (defect #96):** the same reset dropped `modules/out/` from the sealed tree —
and this time no byte-exact copy survived anywhere (repo 404-private, `.git`/`target/` gone at
the boundary, the checkpoint zips are changed-file overlays that never carried the file, the
sandbox's own undo snapshot applies the same exclusion). The manifest was REBUILT per #94's
remedy from its generated doc + the `OutMain` implementation and PROVEN semantically identical
at both layers — `module_docs.py` regeneration byte-matches the checked-in
`docs/modules/out-main.md` (24/24) and the real Rust `decode::decode` validates all 24
manifests — its header declares the reconstruction, and the stamp was re-written: the file's
row moved (`86608e10…`, 4 087 B; comments are the only difference from the sealed `3b0a619f…`,
2 544 B) while the fp line did NOT (`src 96f/2694477B` walks `.rs` files only), so every
figure quoted here and in the pack stays true. The durable recovery copy at
`../sparq-recovery/` is re-established with the reconstructed bytes. **Defect #95 (new,
PRE-EXISTING):**
`ui::live::tests::a_param_edit_crosses_the_command_ring_without_a_restage_and_the_level_follows`
fails deterministically on the PRISTINE clone (stash-proven): the sine amp's 25 ms glide lags the
test's 4-block window. Operator triage owed — move the window or snap the glide; a silent fix
either way would be a lie. **Mutation-stress provenance:** the handoff's recorded 10 000-mutation
hash `7bb06379bd6845e5` does NOT reproduce on the pristine clone on this box; pristine-vs-rebuilt
here is **`8143e1ddfd8fb261`**, stash-proven identical (round 4 held it). Future rounds on this
box use the new baseline; the old figure is the lost sandbox's product (its rustc/platform).

**Eleventh session, third round (2026-10-01, sandbox) — operator UI round 3: control wires, the
junction bus, and six more rulings. BUILT, GREEN and SEALED (`sparq-update-2026-10-01.zip`
rev 3).** The operator's third list, deepest first: **every float parameter is now a controllable
input** — while a cv drag is in flight the hovered module wears a small blue dot beside each
float setting (half a port's radius, the ports' column at the row's height, drawn only in
flight); dropping on a dot makes a dashed control wire the executor honours per block
(`clamp(knob + cv × half-range)`, one block of latency, declared), with verdicts in words (cv
sources only, float sinks only, cycles refused, one modulation per knob, replaced atomically);
**`util/mult`** joins as the six-dot junction bus — role and type set by the first connection,
one source per bus, refusals in words, dots wearing their role, and the bridge COLLAPSES it so
the kernel runs direct source→destination edges (the copies light like the original); **`util/vca`**
(audio gain = knob + bipolar cv, the glide discipline); the **sequencer's pattern is sixteen
step buttons** on the card and 2×8 in the inspector (tap toggles the bit through the param
door); the **scope accepts any audio/cv/event/data source** (the kernel never sees the binding;
audio outputs now publish their first channel's waveform on the analysis ring; event/data
bindings say NO WAVEFORM in words) and its trace wears the SOURCE's class colour — an LFO on the
scope draws in control blue, which is also the lfo well's colour from round 2; **zoom stops at
100 %** (token `zoom_max` 4.0 → 1.0); and card content already scales with the camera from
round 2. First-party set: **21 modules**, docs regenerated 21/21. **Measured: 832 tests (+7:
five r3 gate tests — exact tick lists stay round 2's, new ones pin the modulation's audibility
and clamp, the bus's role/type/source refusals, the collapse-and-light levels, the step-button
flips — plus two smokes), goldens untouched (selftest 9/9, hash ba577186c988db21), every clippy
cell (workspace, ui, ui-window, bootstrap-audio), every python gate, `ui --audit` PASS — matrix
plus 43 checks including the two new round-3 smokes, 0 failures.** Sandbox note, stated not
buried: a reset between turns wiped `/tmp`, `/usr` additions and the workspace `.git`; the
toolchain now lives INSIDE the workspace (`.tc/`, gitignored) and rev 3 ships as an overlay pack
without a patch base (no common ancestor commit survives here) — the stamp still reads
`sync-wo014-inc7c` until the source machine re-writes it after applying. **Device ask, four
lines: wheel-zoom/right-drag/DEL with the real mouse; drag an LFO onto a gain knob's dot and
hear the knob become a bias; clock → seq → percussion, listened; and a mult split lighting like
its source.** Next session: (1) re-stamp + test006 re-run with rev 3; (2) device digests (test004
attempt 4 still the WO-006 acceptance); (3) the operator's eye against `docs/ui/shots/`; (4) the
bare-required badge (item 8) stays open.

**Eleventh session, second round (2026-10-01, sandbox) — operator UI round 2: the clock family
plus eight more rulings. BUILT, GREEN and SEALED (`sparq-update-2026-10-01.zip` rev 2).** The
operator's second list, in the same compute-then-draw discipline: sine AND polyblep narrow to the
audible **10 Hz – 10 kHz** (log map unchanged); two NEW first-party modules take the set to 19 —
**`mod/clk`**, the free-running tempo clock: four sample-accurate trigger grids (4ths/8ths/16ths/
32nds) that share the downbeat, a tempo edit re-spaces ticks FROM THE NEXT ONE (no jump, no
double fire), 32-byte state lands the grid mid-stream — and **`mod/seq`**, the **3–16 step
trigger sequencer**: the clock walks a step ring per qualifying trigger, the pattern bitmask
fires the masked steps AT THE INPUT EVENT'S SAMPLE (pattern, not time), gate-offs ignored; five
new gate tests pin exact sample lists, the wrap, the tempo re-space and zero allocations in
`process`; **multi-choice settings are button rows** (int domains of 3…8 choices — svf mode, lfo
shape, scope colormap, clk-div multiply: one button per choice, tap sets, drag steps; wider ints
stay sliders); **the card-slider dead zone is dead** — a port's 24 px capture no longer reaches
inside its own card (a press inside a body belongs to the body; regression test on both row
ends); **card content is world geometry** — text, knobs, ports, switches, wells and strokes
scale with the camera zoom while touch targets and the selection frame stay screen-sized; the
**Main Out info band clips at char boundaries** (a device name sliced a multi-byte `·` — a panic,
now an ellipsis) and its second line is composed to FIT (`48 kHz · 2 ch · 64 fr · f32`), and
main's input is **stereo** like its output (mono sources still land via the documented
conversion); the **control module's wave display is blue** (the lfo period well draws in the
control-class accent at signal weight, not hairline grey); and **deleting keeps the chain
connected** — the removed node's feed re-aims onto its feedees, paired in port order, each
splice through the same `connect::resolve` verdict a drawn wire gets, removal plus splices in
ONE Batch (one undo restores everything; the shell says how many wires it spliced). The review
sheet now rigs tap → scope AND clock-16ths → seq, so the sheet shows the scope hot with its
graticule and measurements beside the live event chain. **Measured: 825 tests (+1 ignored),
goldens untouched (selftest 9/9, hash ba577186c988db21), every runnable clippy cell, every python
gate, `ui --audit` PASS — matrix + every smoke, 0 failures.** Stamp still `sync-wo014-inc7c` on
purpose: re-write on the source machine after applying, then re-run test006. **Device ask, three
lines: wheel-zoom and right-drag with the real mouse; a fast trim drag; and clock → seq → a
percussion voice, listened.** Next session: (1) re-stamp + test006 re-run; (2) device digests
(test004 attempt 4 still the WO-006 acceptance); (3) the operator's eye against
`docs/ui/shots/`; (4) the bare-required badge (item 8) stays open.

**Eleventh session (2026-10-01, sandbox) — the operator UI round: fourteen rulings, BUILT,
GREEN and SEALED (`sparq-update-2026-10-01.zip` / `.patch`, commit `19fd65b`).** The operator's
UI list arrived as one round and every item landed in the compute-then-draw discipline (sparq-ui
owns the geometry, the egui half only paints it): the sine's slider follows the mouse (Hz params
with a positive minimum map x to value LOGARITHMICALLY, `value_from_x`/`knob_x` one inverse pair;
sine's range is now 0.1 Hz - 10 kHz in the manifest), covered ports are untouchable AND ports
float 4 px beside the window (`canvas.node_port_offset`, draw-order occlusion in `hit_test`, two
new layout tests), the card stripe is 2 px, the selection highlight is WHITE (the token set's one
documented pure-white exemption - `forbidden.pure_white_allowlist`, enforced both ways by
token_gen check 7), the library is 32 px tiles (colour ref + name) under one toggle-switch chip
per module group (the cycling category button is gone), binary settings (bool, or an int whose
domain is [0,1] - the Mute/Mode shape) are TOGGLE BUTTONS on cards and inspector (tap flips, drag
flips once, one undo step per flip), the workspace's right-edge IN/OUT master strip is GONE,
`out/main` is PERMANENT (never spawned, duplicated or deleted - every door refuses in words, four
new interact tests) and reads the session's negotiated driver truth on its own info band
(`backend - device` / `rate Hz - ch - block fr - 32-bit float`; the at-rest sentence when no
session), `dsp/scope`'s screen is 160 px with a 10x8 graticule and a measurement line (ms/div,
Vpp, RMS, PK - `scope::measure`/`graticule`, computed from the SAME window the trace draws, NO
SIGNAL in words when empty), and the mouse hand got its bindings: the scrollwheel ZOOMS the canvas
about the cursor (panels keep their scroll - a pinch over a panel is still declined, its
sequential-contact wobble must move neither camera nor scroll), right-DRAG pans, a still right
press is the context menu as before, and DEL deletes the selection with its protections speaking.
`docs/ui/gestures.md` grew the rules (new 2b the mouse hand, 3c floating ports + covered-is-
untouchable, 3d the permanent Main Out, 4c log-map + toggle bullets, 4e the driver window); the
review convergence sheet now rigs tap to scope so the enlarged scope screen reads hot, and the
round's screenshots are committed under `docs/ui/shots/`. **Measured: 818 tests (+10, +1 ignored),
goldens untouched (selftest 9/9, golden hash ba577186c988db21 - the DSP never moved), every
runnable clippy cell (workspace, ui, ui-window, bootstrap-audio), every python gate, `ui --audit`
PASS - the 5-viewport x 4-DPI matrix plus every smoke, 0 failures, 63 audited elements at
desktop.** The stamp is deliberately NOT re-written in the sandbox: SYNC-STAMP.txt still reads
`sync sync-wo014-inc7c` / `fp 96f/2381733B`, and the pack carries no stamp - on the source machine
run `python tools/sync_check.py --write` after applying (session-start discipline: build.bat will
print the 24 changed filenames before cargo; that report is EXPECTED, not a failure). **Device
ask, two lines: wheel-zoom and right-drag pan with the real mouse, and one fast trim drag to hear
that the toggle/log-map edits still glide.** Next session: (1) re-stamp + session-start discipline
(expect the inc7c stamp until then); (2) device digests (test004 attempt 4 is still the WO-006
acceptance - do NOT fire the ADR-008 exit early); (3) the operator's eye on the new library and
the scope screen against this round's shots; (4) the bare-required badge (item 8) stays open.

**Tenth session, seventh round (2026-09-30, sandbox) — WO-014 increment 7c: the one-pole glide.
BUILT, GREEN and SEALED (`sync-wo014-inc7c.zip`).** Operator, third ear-report: *"I can still
hear bumps when moving the trim slider quickly."* Mechanism named: 7b's linear ramp reached its
target in one block (1.3 ms) and HELD until the next UI snapshot (~16 ms) — a staircase under a
fast drag, kinks at the update rate, i.e. bumps. The ramp is now a one-pole GLIDE (tau 25 ms,
`dsp::core::glide`): the coefficient chases the moving target per sample (per frame on the
multi-channel modules), smooth inside every block, continuous across every boundary; a −80 dB
snap lands settled edits exactly, so static AND settled renders remain the constant multiply —
goldens bit-exact, prime rule unchanged. New gate drives a faster-than-any-hand drag (trim
1.0 → 0.0, one edit per block) and asserts no boundary jump beyond the signal's slope,
monotone block peaks, per-frame stereo coherence, exact silence when settled; the gain unit
test asserts the glide contract; live smoke 27 pumps past two taus before asking whether the
level arrived (the lag is declared fader feel, param_ramp gates its shape). **Measured: 808
tests, release goldens bit-identical, selftest 9/9 · 17/17 · 434.8× realtime, every runnable
clippy cell, 5 python gates, `ui --audit` PASS (50 smokes).** **Device ask, one line: drag the
trim FAST again — staircase kinks were the bumps; the glide has none.** Next session: (1)
session-start discipline (expect the inc7c stamp); (2) device digests (gates at 808 / the inc7c
stamp; test004 attempt 4 still the WO-006 acceptance — do NOT fire the ADR-008 exit early);
(3) the bare-required badge (item 8); (4) the operator's eye on inc6 vs the PN reference.

**Tenth session, sixth round (2026-09-30, sandbox) — WO-014 increment 7b: the stereo master's
ramp fix. BUILT, GREEN and SEALED (`sync-wo014-inc7b.zip`).** The operator's second report —
*"the main out trim still crackles, using a pure sine tone"* — named a real bug in increment 7's
first draft: the `out/main` ramp advanced once per CHANNEL inside the `stereo_tick_f` closure
(it runs per channel per frame), so a stereo change block swept twice its distance and snapped
back to the target at the boundary — a step per block under a drag, the crackle surviving on the
reported knob. One condition fixes it (advance on the frame's first channel); the new gate in
`tests/param_ramp.rs` recovers the per-sample coefficient from the deterministic sine and pins
the change block's geometry (starts at the old coefficient, ends AT the target, no overshoot,
boundary carries only the signal's slope) — on the buggy code it fails by a mile. Mono modules
and the mixer/panner frame loops were already correct. **Measured: 808 tests (+1), release
goldens bit-identical, selftest 9/9, every runnable clippy cell, 5 python gates, `ui --audit`
50 smokes unchanged.** **Device ask, one line: drag the main-out trim again with the sine —
this time the stereo glide is the one under test.** Next session: (1) session-start discipline
(expect the inc7b stamp); (2) device digests (gates at 808 / the inc7b stamp; test004 attempt 4
still the WO-006 acceptance — do NOT fire the ADR-008 exit early); (3) the bare-required badge
(item 8); (4) the operator's eye on inc6 vs the PN reference.

**Tenth session, fifth round (2026-09-30, sandbox) — WO-014 increment 7: the zipper-noise fix
(defect #85). BUILT, GREEN and SEALED (`sync-wo014-inc7.zip`).** Operator report from the device
run: *"when adjusting the main out volume or sine amplitude the audio crackles."* Mechanism:
param snapshots land at block boundaries and every gain-like module multiplied the whole next
block by the new coefficient — a hard step of Δcoeff × signal, 750×/s during a drag. Fix per
`WO014-INC7-PLAN.md`: `dsp::core::coeff_ramp` — the change block GLIDES (linear, one block), the
static path is the zero-step constant multiply, so every checked-in golden stays bit-exact; the
first block after construction primes at the target (no fade-in from a guessed coefficient).
Applied to `syn/sine` amplitude, `out/main` trim, `util/gain`, `util/mixer` (20 coefficients),
`util/panner` (both law coefficients). **Mute keeps its immediate exact zeros — a stage cut is a
safety, not a fade (declared).** Frequency steps, SVF coefficients and cv-side gains are NOT
ramped (slope changes, not amplitude discontinuities; an SVF coefficient ramp is a design of its
own) — parked in LATER with the reason. **Measured: 807 tests** (+3: `tests/param_ramp.rs` — the
boundary-jump gate with its own counterfactual proving the gate would have caught the old step,
the mute-cuts-in-zeros gate, the prime/static bit-identity gate; plus the gain unit test
rewritten to the glide contract), release goldens bit-identical, selftest 9/9 · 17/17, every
clippy cell, 5 python gates, `ui --audit` unchanged at 50 smokes (display-side untouched).
**Device ask: drag the main-out trim and a sine amplitude again — the crackle is the thing that
must be gone.** Next session: (1) session-start discipline (expect the inc7 stamp); (2) the
device digests (gates at 807 / the inc7 stamp; test004 attempt 4 still the WO-006 acceptance —
do NOT fire the ADR-008 exit early); (3) the bare-required badge (item 8); (4) the operator's
eye on the inc6 shell vs the PN reference.

**Tenth session, fourth round (2026-09-30, sandbox) — increment 6: the Persistent-Nodes
convergence. BUILT, GREEN and SEALED (`sync-wo012-inc6.zip`).** The operator's reference:
kageproduction.com/persistentnodes — rulings via question round: warm-shift the tokens (class
hues stay), PN-style node cards with INLINE param sliders, all three surfaces (NODE LIBRARY
sidebar with search + category filter + miniature preview cards replacing the dock palette; the
FIT/RESET/ARRANGE/zoom/wire-style/counts toolbar; right-edge vertical master IN/OUT meters),
project buttons parked. Shipped as WO012-INC6-PLAN.md D1–D7: the warm ladder + orange chrome
accent through `colors.toml` (mockups re-hexed, `make_display_sheet.py` now token-bound so a
palette move can never re-freeze it); the card anatomy (class stripe, category+id header, param
rows with round knobs — one slider vocabulary everywhere — well band, port band, `+N MORE` cap
at 8 rows); `Hit::Param` puts the inspector's edit path on the card (one op, two surfaces); the
library column in `shell::compute` with the reflow order inspector→library→rail (at 1440/1280
the inspector yields — the card is the editor now — collapsing the library brings it back);
ARRANGE is one undo step (Batch of moves); the wire-style select restyles painter AND hit-test;
the master strip reads the ring (IN = the port feeding the master, pre-trim). **Measured: 804
tests (+1 ignored), 50 smokes, every golden bit-identical, selftest 9/9 · 17/17 · 411.1×
realtime, fmt, clippy every runnable cell, 5 python gates, `ui --audit` PASS (matrix 0
violations incl. the library column and the new dense controls, DPI-invariant).** Convergence
sheet pairs the PN screenshot with the shell: `design/mockups/convergence-wo012-inc6.png`;
findings 25–28 declare the deltas. Two build-time decisions recorded in the plan postscript
(reflow order; the fuzzy ranking's looseness embraced). **Next session, in order:** (1)
session-start discipline (`sync_check --quiet`, expect the inc6 stamp); (2) device digests:
gates at 804 / 50 smokes / the inc6 stamp, test004 attempt 4 (still the WO-006 acceptance — do
NOT fire the ADR-008 exit early), and the operator's eye on the new shell vs the PN reference;
(3) the bare-required badge (item 8); (4) parked: project save/open/history, the library port
filter, a third wire style, LOG-tab scroll, Perform's return.

**Tenth session, third round (2026-09-30, sandbox) — increment 5c: Perform mode removed, the
shell loads into Design. BUILT, GREEN and SEALED (`sync-wo012-inc5c.zip`).** Operator ruling:
"when the app loads it should load directly into design mode, remove the perform mode for now."
Shipped: `ShellMode`/`mode`/`set_mode`/`ToggleMode`/the mode buttons/the Perform pads/the
Perform audit rule/the below-breakpoint Design refusal are all GONE; small viewports reflow
Design (inspector → rail collapse for the canvas floor) and say it once in words; the audit
matrix is one mode with a SMALL-REFLOW cell (0 violations); gesture smoke 2 now asserts the
shell loads into Design with no mode door anywhere. **Measured: 804 tests (+1 ignored; the
Perform audit test left with its subject), 44 smokes, fmt, clippy in every runnable cell (MSVC
`ui-window` still the declared sandbox OOM), 5 python gates, selftest 9/9 · 17/17 · 420.3×
realtime, goldens bit-identical.** **Recovery, declared:** a sandbox storage wipe (mid-round,
toolchain and `/tmp` too) destroyed `modules/out/main/sparqmod.toml` — a directory named `out`
is on the snapshot exclude list, so no snapshot ever carried it — plus `.git` and the upstream
clone (GitHub now 404s). The manifest is RECONSTRUCTED field-identical from the generated doc
(`module_docs --check` passes against the doc the original produced) and the module's contract;
its sha moves off `725bf07a…`, the file travels IN this bundle so every tree agrees afterwards,
and SYNC.md names the move. **Next session, in order:** (1) session-start discipline
(`sync_check --quiet` FIRST, expect the 5c stamp and 156 covered files); (2) the bare-required
badge (item 8); (3) device digests: test004 attempt 4 (still the WO-006 acceptance — do NOT
fire the ADR-008 exit early), gates at 804 / 44 smokes / the 5c stamp, and a look at the
small-viewport reflow on the stage screen; (4) parked: Perform's return (LATER), D1′ live
cv-history overlay, second curve module, LOG-tab scroll.

**Tenth session, second round (2026-09-30, sandbox) — increment 5b: the operator's feedback
round BUILT, GREEN and SEALED (`sync-wo012-inc5b.zip`).** The operator ran increment 5 and ruled
on four things, all now shipped code: (1) **meter bars are `out/main`'s alone** — the increment's
audio-output fallback and the analyser cv bars are withdrawn (the ring still carries every port;
the painter spends it on the one meter); (2) **playback is never refused for a bare required
input** — the WO-008 inc-7 build refusal became a FLAG: the executor collects
`missing_required`, the module renders silenced, the canvas paints the light-red highlight
(error tint 0.12 + error border + `NO IN` word, error ring at Dot), PLAY says one sentence
naming every flagged node; (3) **the inspector's no-curve description box is gone** — the plot
well is an `Option` reserved only for curve modules, no box and no words otherwise; (4) **the
log moved to the dock's LOG tab** — the canvas band is deleted, the dock's sixth tab is live and
shows the log tail, tabs are real audited controls now. **Measured: 805 tests (+1 flag model,
−1 withdrawn cv-bar test), 44 smokes (+2: flag/PLAY, LOG tab; 38 and 41 rewritten), every golden
bit-identical (the executor change is build-side only), selftest 9/9 · 17/17, fmt, every runnable
clippy cell, 5 python gates, `ui --audit` PASS (matrix 0 violations, DPI-invariant).** Sheet
re-rendered with a bare `util/delay` in the review state so the flag shows; mockup-review
finding 23 records its encoding. Sealed at the new stamp (below); log_check BASELINE moved only
where numbers moved (stamp; tests stay 805). **Next session, in order:** (1) session-start
discipline (`sync_check --quiet` FIRST, expect the 5b stamp); (2) the bare-required-node badge
(item 8) — note the flag now covers its substance visually, the badge item is the dock/card
half; (3) device digests: test004 attempt 4 (still the WO-006 acceptance — do NOT fire the
ADR-008 exit early), gates at 805 / 44 smokes / the 5b stamp; (4) parked halves: D1′ live
cv-history overlay, a second curve module, LOG-tab scroll (the tail view ships instead).

**Tenth session (2026-09-30, sandbox, fresh clone) — WO-012 increment 5 BUILT, GREEN and
SEALED.** Session-start discipline clean on the fresh clone (`sync_check --quiet` OK at
`wo012-inc4`, 777/0/1 re-verified before any change; `/opt` toolchain reinstalled, MSVC target
added). **Convergence slice B is built to [`WO012-INC5-PLAN.md`](WO012-INC5-PLAN.md)** (D1 the
well registry in `sparq-ui::canvas::inset` — meters live on EVERY audio-output node, cv bars on
the analysers, envelope/lfo/curve param wells; D2 `SvfFilter::magnitude_at` in sparq-audio, the
exact z-domain response pinned to the sine sweep by the drift gate, dispatch in the bridge; D3
the inspector RESPONSE well with its read-only probe marker — tap-to-place, drag-to-read,
≥ 44 px audited, display state, never a second cutoff door, NO RESPONSE CURVE in words where
undeclared; D4 the model split: `canvas::response` + `canvas::inset`, toolkit-independent,
unit-tested). **Measured: 805 tests (+1 ignored), 42 smokes, every golden bit-identical
(`ba577186c988db21`, `dd975a24f03b19c1`, stress `7bb06379bd6845e5`, determinism
`0f5c3e86c7f117a9`), selftest 9/9 · 17/17, probe_alloc 0, 464.4× realtime, fmt + every runnable
clippy cell (MSVC `ui-window` still the declared sandbox OOM), 5 python gates, `ui --audit`
PASS with the marker capture measured in the matrix, DPI-invariant.** Visual review: new
`sparq ui --svg-out PATH --review` door dumps the slice-B showcase state; sheet at
`design/mockups/convergence-wo012-inc5.png`; deviations declared as mockup-review findings
18–22. Two lessons recorded in the plan postscript: a bare `flt/svf` is an illegal LIVE patch
(required input — the meter smoke runs before the showcase spawn, the review state wires
gain → svf), and the svg dumper closed open paths with a stray `Z` (fixed). **Sealed
`sync-wo012-inc5.zip` (24 entries)**; stamp moved once, at completion: **`src 96f/2310417B`**;
log_check BASELINE moved (805 / 464.4). **Next session, in order:** (1) session-start
discipline (`sync_check --quiet` FIRST, expect 155 files at `wo012-inc5`); (2) **the
bare-required-node badge (item 8)**; (3) when the digests land: move the device boxes — test004
attempt 4 is still the WO-006 acceptance (ADR-008 exit: do NOT fire it early), and the gates
digest at 805 / `src 96f/2310417B` / 42 smokes; (4) the parked halves: D1′'s live cv-history
overlay and the second curve module (LATER).

**Superseded — ninth session (2026-09-30, sandbox):** session-start discipline run clean on the
previous tree; step (2)'s plan of record WRITTEN (`WO012-INC5-PLAN.md`, docs-only at the time).
The build it planned is the tenth session's entry above.

**Next session, in order:** (1) session-start discipline — `python3 tools/sync_check.py
--quiet` FIRST; if `modules/out/main/sparqmod.toml` reports MISSING restore it from
`../sparq-recovery/modules-out-main-sparqmod.toml` and verify sha `725bf07a…`; the `/opt`
toolchain is gone (recipe in the environment notes; ~150 s warm). (2) **The next sandbox build
is convergence slice B — WO-012 increment 5: node inset displays + the inspector response
plot** (LATER §WO-012 increment 5+; the scope well from WO-013 inc 6 is the pattern; the
mockup's per-module wells — envelope triangles, meter bars, sparklines — draw from LIVE ring
values where the rings carry them and sit at rest otherwise; the response plot needs a
per-module curve contract, svf first, declared before drawn). Plan of record first, the
WO008-INC7 discipline — **DONE this session: build to [`WO012-INC5-PLAN.md`](WO012-INC5-PLAN.md)**
(D1 well registry · D2 svf curve contract · D3 response plot + probe marker · D4 model split);
seal only when the increment is COMPLETE and green (a partial increment is not sealed). (3) Then the bare-required-node badge (item 8). (4) When the digests
land: move the device boxes; and if test004 attempt 4 passes — WO-006 acceptance closes and
the ADR-008 exit fires (bootstrap deleted, HAL becomes `play`'s default): do NOT do it early.

**Visual review pipeline (new this session, for slice B):** `sparq ui --svg-out PATH --width
2560 --height 1600` dumps the headless frame's vector shapes; render with the sandbox's
`/opt/arena-python/bin/resvg` AFTER substituting the monospace family for `DejaVu Sans Mono`
(fonts-dejavu-core + fontconfig are installed; without the substitution resvg renders no
text); compare against `design/mockups/design-mode.svg` the same way — the standing sheet is
`design/mockups/convergence-wo012-inc3.png` (mockup over shell).

---

**Earlier this session (eighth session, first increment — the device blocker cleared and the
operator's four asks shipped as WO-012 increment 4**: (1) **the COM/APARTMENT
fix** — the operator's first PLAY on SATURN refused with `this thread already initialised COM as
single-threaded (STA); the WASAPI HAL needs MTA`; the session now spawns ONE dedicated
audio-control thread per session (MTA by simply calling the HAL), the only thread that touches
the backend — enumerate/probe/open/start/stop/drop/pump/fault/capture ride a request-reply
protocol with bounded waits, the UI thread keeps the `SharedEngine` control half and a polled
health mirror, and `Drop` stops + joins (a session that leaked its thread would leak the
device); (2) **mouse usability** — unpressed mouse motion is dropped at the window adapter (the
suppression line can now only mean a real missed Down), right-click synthesises the
recogniser's own `Context` intent, the wheel synthesises `Pan` (the panel-over-panel routing
reused — inspector scrolls over the inspector, camera pans over the canvas, no modifier keys
anywhere); (3) **`out/main` meter bars like the mockup** — `MeterUpdate` grew `peak_l`/`peak_r`
(one cache-warm pass; mono ports duplicate, pinned), the bars draw green DATA-class fills with
amber AUDIO-class peak-hold blocks decaying on AUDIO time (block counts, no wall clock), empty
wells at rest; (4) **the palette** — dock tiles 112×56 (class M, floor holds), grouped by
manifest top category under xs words, left stripe in the dominant signal class (§4: colour says
what it carries, the group says what it is), all seventeen fit at 1920. **Sandbox-green: 777
tests** (+2 session gates behind `ui`: back-to-back sessions + drop-without-stop, the stereo
pin), **37 smokes** (+3: dock-card spawn/undo stayed, right-click menu, wheel routing + hover
silence, master bars), every golden bit-identical (stress `7bb06379bd6845e5`, exec renders,
`canvas-render.wav`), selftest 9/9, 17/17, every clippy cell incl. MSVC×6 + native ui-window
(the window adapter changed), 5 python gates (R6 caught nothing new; the disallowed-`Mutex`
lint got a reasoned control-path allow), stamp **`src 94f/2188595B`**, log_check BASELINE moved.
Sealed **`sync-wo012-inc4.zip`** — the ONLY waiting bundle now. **Device ask: tap PLAY again —
sound is the acceptance**; then test004 attempt 4, test006 F–K (+ the mouse paragraph), gates at
777 / `src 94f/2188595B` / 37 smokes, digests back. **Next sandbox build: convergence slice B**
(node inset displays + the inspector response plot), then the bare-required badge.)

**Previous handoff (seventh session, third increment — the operator's visual ask shipped as
WO-012 increment 3 — mockup conformance, chrome**: the shell now draws
in `design-mode.svg`'s language, measured side by side with the new `sparq ui --svg-out`
instrument (a headless vector screenshot — the review protocol's first repeatable visual
comparison; sheet at `design/mockups/convergence-wo012-inc3.png`). **What converged:** the rail
(glyph + permanent micro-label in the mockup's own 44 px box — the operator's ruling between
the mockup's icon-only rail and look-board §8; grouped by hairline dividers; the foot carries
the live level tick in the AUDIO class colour and the magenta ADD cross that opens the browser
at canvas centre), the top bar (logo glyph, section dividers, the mockup-review's "top-bar
diagnostics" line — negotiated kHz · frames · xruns · swaps while live, the build stamp at
rest — and the status dot paired with its word), the wire-encoding legend floating top-right
(drawn by the canvas painter from the wires' OWN encoding table), the dock (the MODULES tab is
REAL now: the registry catalogue as 248×80 class-dot cards — tap spawns at canvas centre
through the browser's own op path, undoable; overflow said in words; the other four tabs stay
honestly disabled), the inspector (port-dot summary strip in manifest order — rings in, filled
out — and the mockup's control-cyan sliders with block thumbs), and the shell's COLUMN
DISCIPLINE (`shell::compute` moved: rail and inspector run full height, the dock lives in the
canvas column — the layout test pins it). The demo patch spreads left-to-right like the
mockup's story patch. **Sandbox-green: 777 tests** (count unchanged — chrome moves no audio),
**34 smokes** (+2 chrome: a dock-card tap spawns and one undo removes; ADD opens the browser
and an outside tap cancels), every golden bit-identical, selftest 9/9, 17/17, every clippy
cell, 5 python gates (R6 caught the SVG dumper's page-ground hex — it now reads the token),
the breakpoint matrix grew with the cards (0 violations, the 44 px floor holding), stamp
**`src 94f/2155888B`**, log_check BASELINE moved. Deviations declared in `mockup-review.md`
findings 9–14 (inspector 400 not 480 — tablet reflow; top-bar word buttons — §6; amber level
tick — §4). Sealed **`sync-wo012-inc3.zip`** — WAITING with inc6/inc2/inc7 for the device runs.
**Next: increment 5 of the shell line — convergence slice B (node inset displays + the
inspector response plot), then the bare-required badge.** Device digests still wanted.)

**Earlier this session (second increment — the scope screen: WO-013 increment 6** — the follow-on WO-012 inc 2 named, the operator's pick. Plan of
record `WO013-INC6-PLAN.md` (twelve decisions before the code; its postscript records the
correction the smoke caught — **D3′**: the prescribed per-frame SWAP of a canvas-owned trace
field alternated two sets and blanked alternate frames under manual pumping; the shipped design
has the SESSION as the single source, read by the painter through a `canvas_ui::draw`
parameter, empty at rest — a dead stream's signal has nowhere to linger). `dsp/scope` draws its
bound tap's waveform from the analysis ring: `sparq-ui::canvas::scope` (rolling traces sized by
timebase × the NEGOTIATED rate, zoom-not-reset resize, rising-edge trigger with the declared
free-run fallback, stride-decimated clamped geometry, X/Y pairing, NaN sanitised, params
clamped without trust); **the wire is the binding** — resolved per frame, impossible to stale,
a rebind clears the traces; **the ring is the payload** — fed from the SAME drain pass the cv
wire levels read; the painter in the `scope.trace` token map and the WIRES' glow vocabulary —
well at Full AND Simplified, TRACE at Full only, Dot untouched; unbound/at rest = the flat rest
line ("a flat line, not a crash" — the manifest's own promise); `Scope::process` stays a no-op
(zero audio-thread cost, structural). No layout move, no manifest move, no new audit elements.
**Sandbox-green: 777 tests** (+11 model gates, +3 session gates, +2 smokes = **32**), every
golden bit-identical (three pinned exec renders, stress `7bb06379bd6845e5` debug AND release,
`canvas-render.wav` `d7ad294e…`, determinism `0f5c3e86c7f117a9`), selftest 9/9,
`modules --strict` 17/17, every runnable clippy cell (the increment owed `div_ceil` ×3 —
clippy caught it), 5 python gates, stamp **`src 94f/2122410B`**, log_check BASELINE moved with
the seal. Sealed **`sync-wo013-inc6.zip` (16 entries)** — WAITING for the device, stacked on
inc2 + inc7. Device expectations move again: gates **777 / `src 94f/2122410B` / 32 smokes**;
test006's counts moved (J–K unchanged in shape — the scope animates on screen during them).
**Next step: DEVICE EVIDENCE — apply inc7 → inc2 → inc6 in order, run test004 attempt 4,
test006 F–K, gates, send the digests. The named next SANDBOX build: the canvas badge for the
bare-required node** (item 8 — the smallest well-specified item now that the scope shipped);
after it, the operator's save/load question (patch persistence, WO-011's drafted `.sparq`
format) is the biggest remaining UX gap for "building patches and sounds".)

**Earlier in this session (increment 2 — the UX pivot's first build: WO-012 increment 2, the
live audio session**, the checklist's ACTIVE item
6, sealed as **`sync-wo012-inc2.zip`** and WAITING for the device, stacked on the still-waiting
`sync-wo008-inc7.zip`. Plan of record `WO012-INC2-PLAN.md` — fifteen decisions before the code,
settling every open question item 6 listed; its postscript records the one decision the CODE
corrected: `stage` never refuses — it supersedes and counts — so the retry flag D8 prescribed
does not exist, and the visible refusal is the BUILD's, in the executor's own words. **What
shipped:** PLAY/STOP/PANIC became a real `LiveSession` (`sparq-app/src/ui/live.rs`) — two-phase
open (probe → executor built for the NEGOTIATED config via the new `bridge::build_with_map_at` →
real open), the callback owns the `AudioEngine` and does `render_block` and nothing else; the
ONE op→sync door (`CanvasState::take_patch_changes`, classified per `Op` exhaustively: params
cross the command ring with ZERO re-stages, structural edits and master handovers re-stage at
the boundary, moves/renames mark nothing); the bounded per-frame drain fills `CanvasState.levels`
from `read_meters` + `read_analysis` — **the continuous half of live wire levels, the painter
path untouched**; the ring extension it rides (`MeterUpdate.port` — one entry per audio OUTPUT
port, `FOLDED` sentinel for cv-only nodes, single-output entries BIT-IDENTICAL to the folded
readings they replace, pinned against an offline reference; block-rate cv publishes as
one-sample analysis waveforms so a cv wire's live level reads the inc-5 magnitude rule); the
honesty lines (NEGOTIATED · latency · the STOP evidence line of measured counters); `Removed`/
`Failed` ends the session in one line, canvas untouched, engine dropped on the control thread
after `reclaim()` drains. Audit + tests run the **manual null** pumped by the smoke (hermetic;
`capture_frames` holds the samples as device-side proof); a Windows window gets `wasapi-shared`;
elsewhere the paced null WITH a line saying so. **Sandbox-green: 766 tests** (+4 workspace: the
per-port ring gate, the block-rate analysis gate, the ledger gate, the config-door delegation
pin; +5 `live.rs` session tests and +5 audit smokes behind `ui`), every golden bit-identical
(the three pinned exec renders, `canvas-render.wav` 1 920 046 B · `d7ad294e…`, stress
`7bb06379bd6845e5` debug AND release, determinism `0f5c3e86c7f117a9`), selftest 9/9,
`ui --audit` **PASS 30 smokes**, `modules --strict` 17/17, every runnable clippy cell, 5 python
gates, stamp **`src 93f/2081612B`**, `log_check.py` BASELINE moved with the seal (#83). test006
grew **steps J–K** (PLAY audible on shared, wires animating while playing, a live slider edit
heard without stopping, the STOP evidence line; optional unplug → the Removed line and an
untouched canvas). **Next step: DEVICE EVIDENCE — apply `sync-wo008-inc7.zip` THEN
`sync-wo012-inc2.zip`, run test004 attempt 4, test006 (now F–K), gates (766 /
`src 93f/2081612B` / 30 smokes), send the digests. The named next SANDBOX build: the scope
screen** (the analysis ring publishes waveforms with nothing drawing them — the plan's declared
follow-on), then the canvas badge for the bare-required node (item 8).)

**Previous handoff (sixth session — SECOND increment; the device applied
`sync-wo006-inc15.zip`** (operator-confirmed; test004 attempt 4 has NOT come back yet — the 🔴
blocker stands, its pass shape in `WO006-INC15-RUN-SHEET.md`). The sandbox then built the
checklist's named next candidate: **WO-008 increment 7 — host-side `required`-unconnected
enforcement**, its own increment on purpose because it MOVES the declared baselines, exactly as
LATER.md conditioned. Plan of record: `WO008-INC7-PLAN.md`
(eight decisions before the code). `Executor::build` now refuses a bare `required` input in
words (`ExecError::RequiredUnconnected` — node, module, port, both remedies); the vocabulary
was already deliberate across the seventeen manifests (six single-input processors required;
every matrix/display/sink/trigger optional), so NO manifest moved and `modules --strict` stays
17/17. The two declared dependents moved with it: the determinism world WIRES its spare (from
the chain gain — the spare is the initial master, so retunes reach the samples), and the canvas
demo's fourth node became a bare `out/main` (the one module designed to sit bare; the master
stays the chain gain and `canvas-render.wav` is bit-identical by construction — measured
1 920 046 bytes, the device baseline's exact size). **The stress baseline moved on purpose and
is recorded: `7bb06379bd6845e5` · 10 001 blocks · 2 383 swaps · 7 617 refused · 0 audio-path
allocations, debug == release** (was `b42068ec7b206789` · 7 203 · 2 797); the cross-thread
ledger moved with it and its status pin TIGHTENED (`Ok` only — `Silenced` would now mean the
enforcement leaked). Sandbox-green: **762 tests**, every audio golden bit-identical (the three
pinned exec renders re-verified at their durations), selftest 9/9, `ui --audit` PASS 25 smokes,
every runnable clippy cell, 7 python gates. Sealed as **`sync-wo008-inc7.zip`** — WAITING for
the device, stacked on the applied inc15. **Defect #93 found and remedied mid-increment:** the
sandbox workspace drops any directory named `out` at message boundaries — `modules/out/main/
sparqmod.toml` (stamp-COVERED) vanished; upstream GitHub 404'd on it too (repo rebuilt/private
— #86's class again); the operator supplied the exact bytes, restored hash-verified against the
seal (`725bf07a…`, 2 691 B); a durable recovery copy now lives at the workspace root's
`sparq-recovery/`, and the environment notes carry the session-start discipline. **Next step:
DEVICE EVIDENCE — apply `sync-wo008-inc7.zip` on top of inc15, run test004 (attempt 4),
test006, gates (expected count is now 762), send the digests.**
**SESSION 6 CLOSE — THE OPERATOR'S UX PIVOT (decisions recorded; NO code written for it yet).**
The operator's call, verbatim intent: "start fleshing out the interface and start building
patches and sounds … focus on the user experience now and come back to the backend refinement
later." Two decisions were made through the question tool and are ratified here: **(1)
RE-SEQUENCING — live canvas audio no longer waits for WO-006's exclusive acceptance.** The
recorded gate existed because of the ADR-008 EXIT (deleting the cpal bootstrap, HAL becoming
`play`'s default), not because of shared-mode live audio: shared is the most device-proven path
in the project (2 h soak / 0 xruns, unplug→`Removed`, recovery, conformance — all on SATURN).
The interim live vehicle is `wasapi-shared` at its negotiated ~22.67 ms — fine for building
patches and sounds, not for stage monitoring. **The ADR-008 exit itself stays gated on attempt 4
exactly as recorded — do not delete the bootstrap early.** **(2) FIRST UX INCREMENT = live audio
+ continuous meters** (sandbox item 6 below, which carries the research notes): the app-side glue
over the inc-5 cross-thread halves, per-frame `read_meters` → `CanvasState.levels` (the painter
path is DONE — "it only swaps the SOURCE"), with per-port meters + ring port ids riding it and
the scope screen (analysis ring) as the named follow-on. The DEVICE track stays warm in
parallel, unchanged and cheap: apply `sync-wo008-inc7.zip`, run attempt 4 (~15 min), test006
F–I, gates (762 / `src 92f/2021628B` / stress `7bb06379bd6845e5`). The pivot cancels no device
box and unblocks no gated deletion. Next session's FIRST act on the UX track: write the plan of
record (decisions before code — the `WO008-INC7-PLAN.md` discipline) for the live-audio
increment, settling the open questions item 6 lists; the environment notes' session-start
discipline (`sync_check --quiet`, the #93 `modules/out` restore) applies as always.
Earlier in this session (increment 1.5, sealed `sync-wo006-inc15`, now APPLIED): the device
moved again — **test004
attempt 3 ran on SATURN** (2026-09-28 evening) — `sync-wo006-inc14.zip` confirmed APPLIED by
its own build line (`sync_check: OK - 151 files match sync wo006-inc14, src 92f/2001521B`) —
and it **confirmed the #89 open-path fix on hardware, then falsified its remaining
assumption**: the exclusive open landed EXACTLY on the predicted pass shape (`i24-in-32
(converting) · device period 960 fr (10.00 ms) · sparq block 64 fr` — the default period asked
first and granted, no shrink note) and the stream STILL stalled with attempt 2's triple at the
new period: 158 late wakes / 10 s (~16 stalls/s, each ~40 ms swallowing ~3 cadences), half
throughput (7756 blocks ≈ 49.4k fr/s against a 96 kHz negotiation), jitter avg 19.44 / max
40.36 ms over 517 wakes, drift −491 161 ppm, no clean tone, rc 1 — with 0 budget overruns and
0 FIFO starvations AGAIN, and every shared-mode step green AGAIN in the same session (unplug →
`Removed`, recovery tone, the 3-min rehearsal soak, all three conformance suites incl. the
exclusive lifecycle/latency/reopen-leak checks on hardware). The arithmetic
(`docs/hal/windows-notes.md` §4e) joins both attempts into one identity — **delivered frames =
wakes × buffer**, exact to one block (517 × 960; 1718 × 288) — because event-driven exclusive
legally cannot bank more than one period (`hnsPeriodicity` ≡ `hnsBufferDuration` under
`EVENTCALLBACK` — MSDN's rule, `AUDCLNT_E_BUFDURATION_PERIOD_NOT_EQUAL`), so every swallowed
cadence is lost audio. The stall (~16 Hz, ~33–40 ms wall) did not move between the 3 ms and the
10 ms period: **period-independent — defect #92, open, device-side.** And attempt 3's own
`adjusted rate 48000 Hz -> 96000 Hz` line exposed **defect #91**: the exclusive rate envelope
had been probed through the SHARED sieve (the engine accepted only its 96 kHz mix), so the
driver was never asked about 48 kHz exclusive and play/soak adjusted inside the same wrong
envelope — UP into the one rate the exclusive path proved pathological at. **WO-006 increment
1.5** was built this session and sealed as **`sync-wo006-inc15.zip` (15 entries)**: the
exclusive sweep is its own probe (every standard rate × the four rungs, asked of the driver);
adjustment consults the envelope the open negotiates in (`Capabilities::exclusive_rate_for`,
pure + Linux-pinned); **#76 SHIPPED** — drift is delivered-frames-vs-wall (the pump counts what
the device ACCEPTED; the `IAudioClock` binding is DELETED, its `GetPosition` was buffer-step
fiction — §4b), both recorded healthy sessions pinned at ≈0 ppm under the new derivation,
attempt 3's shape pinned as the −49 % verdict it is, suspect drift (> ±1 % after a 2 s trust
window) now FAILS play/soak verdicts in its own right; and `test004.bat` is current for
**attempt 4 at 48 kHz** — with the acceptance-wording amendment (soak at the rate the device
proves; 96 kHz on offer as the stretch) PROPOSED in the run sheet §2, not smuggled.
Sandbox-green: **761 tests**, goldens bit-identical (the three pinned exec renders included),
selftest 9/9, `ui --audit` 25 smokes, every runnable clippy cell clean — superseded this
session by increment 7's 762; the bundle was applied on the device as reported above.
Previous handoff (fifth session): **WO-006 increment 1.4** built and sealed after test004
attempt 2 opened exclusive at the driver's 3 ms minimum and stalled (**defect #89** — the third
lying-`min` face: `Initialize` says yes, runtime says no): the default-first period ladder for
coarse engines, the round-UP alignment two-step, the silent-shrink guard with the honest open
line — all pure data in `hal/period.rs`; plus **defect #90** (test004.bat's summary
distinguishes refused from opened-but-not-clean) and **`tools/log_digest.py`** (+ `digest.bat` +
end-of-run hooks): the compact `-digest.log` copies — verdict lines byte-identical, repetition
collapsed — and the digest is the send-back artefact. 757 tests, both MSVC `hal-wasapi` cells
clean. Attempt 3 then DEVICE-VERIFIED the open-path half of #89 (the open line landed on the
predicted 960 fr shape) and reclassified its runtime half into #92.)

---

## Current position

- **Device state:** every sealed bundle through **`sync-wo006-inc15.zip` is APPLIED on SATURN** —
  inc15 by operator report (2026-09-29; test004 attempt 4 has not come back yet), everything
  before it proven by attempt 3's own build line (`sync_check: OK - 151 files match sync
  wo006-inc14`). **Operator-reported 2026-09-29/30: the four bundles through
  `sync-wo012-inc3.zip` are APPLIED — the device carries the live audio session, the scope
  screen and the converged chrome; the first PLAY then refused on the STA/MTA apartment
  clash, which increment 4 fixes. **`sync-wo012-inc4.zip` APPLIED 2026-09-30 and
  DEVICE-VERIFIED: PLAY MAKES SOUND (operator report, end of eighth session).** No bundle
  waits. The digests (test004 attempt 4, test006 F–K incl. the mouse paragraph, gates at 777 /
  `src 94f/2188595B` / 37 smokes) are STILL WANTED.** Once it lands, the device runs cover thirteen increments of
  baseline — **`test004` attempt 4 (exclusive acceptance at 48 kHz — still the 🔴 blocker; send
  back `test004-digest.log`; pass shape + the PROPOSED acceptance-rate amendment + the
  failure-reading guide in `WO006-INC15-RUN-SHEET.md`)**, `test006` (canvas window session —
  the F–I asks stand, and **steps J–K are NEW with inc2 — the live audio acceptance: PLAY
  audible on wasapi-shared, wires animating WHILE it plays, a live slider edit heard without
  stopping, the STOP evidence line, optionally an unplug mid-play**; NEW with inc7: the demo
  patch's fourth node is a bare `out/main` instead
  of the spare Gain, and `canvas-render.wav` is byte-identical to the recorded baseline —
  1 920 046 B, sha256 `d7ad294ea0b5e6e9…`; step [05]'s A/B is unchanged in shape), `gates.bat`
  (**expected test count is now 777**, stamp = whatever `SYNC-STAMP.txt` says (`src 94f/
  2188595B` — the bundle's own line is truth), and the stress evidence line in the test output
  reads **`7bb06379bd6845e5` · 2 383 swaps · 7 617 refused** — the moved baseline, not a
  regression), `sparq modules --strict`
  lists **17**, `selftest --golden` prints **PASS (9 gates)**, `ui --audit` **PASS (37 smokes)**;
  `log_check.py`'s BASELINE moved with the seal, #83's discipline), and the WO-008 **loaded
  soak** (200 modules, 30 min) when the HAL and the machine are both free. WO-009's two device
  boxes: the **30-minute drift measurement** — now riding the NEW delivered-vs-wall metric; read
  its box against #76's shipping note before comparing with any old log — and a **listened tempo
  sweep**. Evidence artefacts — **defect #85: run them with their pinned durations or the hashes
  will not match the goldens** (the CLI default renders 5 s): `sparq exec --patch drum-demo
  --seconds 2` (transport-driven, hash `f2303f13aa0cf299`), `--patch mod-demo --seconds 1`
  (`1621e1f65b1b64e1`), `--patch demo --seconds 2.8` (`53de3b1f3f40e3c9`) — all three
  re-verified bit-identical in the sandbox this session (inc 1.5 touches no render path).
- **WO-008 increment 7 is BUILT and sandbox-green — host-side `required`-unconnected
  enforcement, the declared baseline moved ON PURPOSE:** the sandbox track's named next
  candidate; plan of record `WO008-INC7-PLAN.md` (eight decisions before the code).
  `Executor::build` refuses a patch with a bare `required` input — `ExecError::RequiredUnconnected`
  names node, module, port and both remedies (connect a source or remove the node; a genuinely
  optional port is a manifest `required = false`), placed after the edge-rules pass (an ILLEGAL
  wire is a more specific defect than a MISSING one) and before buffer allocation, graph order ×
  manifest order so the first violation is deterministic. The rule's scope is the compiled
  contract's own words (`Port::required` = whether an unconnected INPUT is an error): outputs
  stay free, one-or-more incoming edges satisfies a port, and cv fan-in was already refused
  upstream so the check never legitimises a wiring the matrix refuses. No manifest moved — the
  seventeen already spoke the vocabulary deliberately (six single-input processors required;
  matrices/displays/sinks/triggers optional) — so `modules --strict` stays 17/17 and no audio
  golden moved (enforcement only ADDS refusals; the demo patches were read and are compliant).
  **The two declared dependents moved with it:** the determinism world WIRES its spare — from
  the chain gain, not the sine, on purpose: the spare is the initial master, so retune mutations
  reach the rendered samples (pin: 4 nodes, 3 edges) — and the canvas demo's fourth node became
  a bare `out/main`, the one module designed to sit bare (optional input; `resolve_master`'s
  documented rule ignores an unwired out/main, so the master stays the chain gain and
  `canvas-render.wav` is bit-identical by construction — measured 1 920 046 B, the device
  baseline's exact size, sha256 `d7ad294ea0b5e6e9…` recorded). The unconnected-signal semantics
  live at their declared home — OPTIONAL ports: the silenced-report test migrated in place
  (probe declares `required = false`), the probe manifests' audio inputs all declare optionality
  (the suite run was the census: 8 tests touched the enforcement, all migrated/reworked in
  place), and the new gate `a_required_input_with_no_wire_is_refused_in_words` proves the
  sentence AND its compliant twin. `cross_thread.rs`'s status pin TIGHTENED to `Ok` only —
  `Silenced` in a meter would now mean the enforcement leaked. **The baseline moved on purpose
  and is recorded: stress `7bb06379bd6845e5` · 10 001 blocks · 2 383 swaps · 7 617 refused ·
  0 audio-path allocations · debug == release** (was `b42068ec7b206789` · 7 203 · 2 797) — the
  mix flipped because add/disconnect mutations that bare a required input are now refusals: the
  leave-no-trace path is exercised 2.7× harder, the swap floor (>1 000) holds at 2 383, and the
  cross-thread ledger moved with it (21 509 blocks · 2 299 staged = swaps = reclaimed · 7 701
  refused · 0 allocs · 0 errors). Docs moved in the same session (#88): module-api-v1's open
  list loses the item, the field-table + schema rows grow the enforcement clause, the executor
  header's v0 bullet is replaced, LATER.md struck through + a new parked item (the canvas badge
  for the bare-required node, WO-013-side, beside the watchdog hairline). **762 tests** (was
  761; +1 refusal gate), 0 failed, 1 ignored; goldens bit-identical (release re-verified: the
  three pinned exec renders, wo005, phase-b, every module golden); selftest 9/9; `ui --audit`
  PASS 25 smokes with the new demo node; fmt + all runnable clippy cells clean; 7 python gates.
  Sealed as **`sync wo008-inc7`** — WAITING, stacked on the applied inc15. **Defect #93** logged
  below (the `out`-dir snapshot trap that ate `modules/out/main/sparqmod.toml` mid-increment;
  recovered byte-exact via the operator, hash-verified against the seal).
- **WO-006 increment 1.5 is BUILT and sandbox-green — the exclusive rate envelope is its own
  probe, and drift reads delivered-vs-wall:** attempt 3's two findings, fixed where the sandbox
  can fix them. **#91:** `capabilities()`' exclusive sweep ran only at rates the SHARED probe
  accepted — on SATURN that left `exclusive 96000` printed as measured truth (the engine refuses
  every shared f32 rate but its own mix) while the "UMC 204HD 192k" driver was never asked about
  44.1–192 kHz exclusive; `play`/`soak` then adjusted inside the same shared envelope, bending
  the standing 48 kHz ask UP to 96 kHz. Now: the exclusive sweep asks the DRIVER at every
  `PROBE_RATES` rate plus the mix rate (four rungs each; `Initialize` stays the final arbiter),
  and a new pure helper `Capabilities::exclusive_rate_for` (`hal/mod.rs`, Linux-pinned: a listed
  rate survives, nearest otherwise, ties to the LOWER rate, empty envelope → `default_rate` so
  the open ladder still refuses with its full probe table) routes both CLIs' exclusive
  adjustments. **#76 SHIPPED (pulled from the increment-2 backlog — attempt 3 made it the
  missing instrument):** the drift metric counts frames the device ACCEPTED (every successful
  `ReleaseBuffer`, preroll included) against the wall; the `IAudioClock` binding is DELETED with
  an epitaph where its vtable was (its `GetPosition` advanced in buffer-sized steps per event
  tick — +1.2 M ppm on a soaked-clean shared stream, §4b — and hid a real half-throughput
  exclusive inside the same SUSPECT label). §4b's fixture requirement is honoured in tests: both
  recorded healthy sessions read ≈0 ppm under the new derivation, attempt 3's shape reads the
  −49 % half-throughput verdict it is, and the null backend (which always counted delivered
  blocks) now shares ONE metric meaning across backends. The falsified "implausible clock
  (virtual/RDP endpoint)" SUSPECT text is replaced by what the number means; `drift_suspect`
  waits for a 2 s trust window (one period of quantisation at 96 kHz/10 ms = ±5 000 ppm over
  2 s = half the threshold — `DRIFT_TRUST_NS`'s doc carries the arithmetic); and the verdicts
  ACT: `play`'s pass line requires `!drift_suspect` (the NOT CLEAN line says so when that is the
  reason), `soak` fail-fasts on it. **#92 stays OPEN, device-side, and named:** the exclusive
  event-stall phenomenon itself (~16/s × ~33–40 ms wall, period-independent, delivered = wakes ×
  buffer exactly — the zero-headroom law of event-exclusive, MSDN's `hnsPeriodicity` ≡
  `hnsBufferDuration` rule, is in the wasapi.rs header now). Attempt 4 discriminates
  rate-coupling; if the triple returns at 48 kHz the next lever is **push-mode exclusive**
  (periodicity 0, no `EVENTCALLBACK`, buffer 2–4 periods, self-paced pump — the shape the audio
  engine itself runs on this endpoint, 2 h clean), parked in LATER.md with its design burden.
  `test004.bat` is current for attempt 4 (multi-rate [03] expectation, the 48 kHz [04] open line
  with NO `adjusted` line, #92 hint vocabulary, the [07] rate prompt defaulting to 48000 with
  96000 as the WO-text stretch). **761 tests** (was 757; +3 diag fixtures — the trust window,
  the attempt-3 replay, the #76 regression pair — +1 `exclusive_rate_for` gate; the
  `implausible_drift` test replaced in place), 0 failed, 1 ignored; goldens bit-identical
  (release re-verified: wo005, demo, the three pinned exec renders at their durations);
  selftest 9/9; `ui --audit` PASS 25 smokes; null conformance 9/9; paced-null play/soak smokes
  read +0.23/−0.62 ppm — the new derivation at ≈0, live; every runnable clippy cell clean
  (default, bootstrap-audio, ui, native ui-window-gles `-j 1`, MSVC×6 incl. both `hal-wasapi`
  cells); 5 python gates + `sync_check --self-test` 11/11 + `log_digest --self-test` 16/16 +
  `log_check --self-test` 25/25 (BASELINE moved: 761 / `src 92f/2015804B`). Sealed as
  **`sync wo006-inc15`**; device contract = `scripts\test004.bat` + `WO006-INC15-RUN-SHEET.md`.
- **WO-006 increment 1.4 is BUILT and sandbox-green — the defect-#89 default-first exclusive
  period ladder + the compact log digest:** `hal/period.rs` reorders (a driver whose reported
  minimum sits ABOVE the sparq block gets asked its DEFAULT period first — the number its engine
  actually runs; the min-clamped ask is demoted to fallback; sub-block engines keep the honest
  low-latency ask first, #79's shape unchanged), the `BUFFER_SIZE_NOT_ALIGNED` two-step rounds
  the ask UP to the driver's granularity (960 fr ask × 288 fr granularity → 1152 fr = 12.000 ms)
  instead of adopting it, and a post-open guard treats an allocation < ¾ of the accepted ask as
  an undeclared granularity — one rounded-up re-ask, else the open line prints the mismatch
  (`(3.00 ms, ask 960 fr)`) rather than dressing it as clean. 6 new Linux unit gates + 1
  replaced in place (the attempt-2 device shape is pinned twice: `the_defect_89_device_asks_its_
  default_before_its_min`, `the_alignment_two_step_rounds_the_ask_up_never_down`); the pump
  untouched (Period ≠ block absorbs any period). **Defect #90** fixed beside it: `test004.bat`'s
  summary said "exclusive still refused" when attempt 2 had OPENED and run rough — it now
  distinguishes refused from opened-but-not-clean (`EXCL_OPENED`), and its banner/[02]/[04]/hint
  texts are current for the first time since inc 1.2. **And `tools/log_digest.py`** (+
  `scripts\digest.bat` + end-of-run hooks in test004/test006/gates): compact `-digest.log`
  copies of the device logs — every verdict byte-identical, three repetition classes collapsed —
  because a 2 h soak's 240 status lines were drowning the dozen lines that decide the run
  (attempt 2: 67 569 B → 30 994 B, −54 %; `--self-test` 16/16). Sealed as
  **`sync wo006-inc14`** — APPLIED on the device 2026-09-28 evening; attempt 3 DEVICE-VERIFIED
  the open-path fix (the open line landed on the predicted `960 fr (10.00 ms)` shape, no shrink
  note) and reclassified its runtime half into #92 (the stall is period-independent). Device
  contract was `scripts\test004.bat` + `WO006-INC14-RUN-SHEET.md`.
- **WO-008 increment 6 is BUILT and sandbox-green — `cv_interp = "spline"`, the G4 vocabulary
  performed in full:** the checklist's sandbox-track item 8, second entry — named there as the
  next candidate; the plan of record is `WO008-INC6-PLAN.md` (eight decisions, recorded before
  the code). **The curve:** the parabola through the last three block values —
  `v(t) = prev2·t(t−1)/2 + prev·(1−t²) + cur·t(t+1)/2`, `t = i/frames` — equivalently a cubic
  Hermite whose start tangent is the central difference `(cur−prev2)/2` and whose end tangent is
  the second-order backward estimate `(3cur−4prev+prev2)/2` (four constraints, one cubic, the same
  curve; the equivalence is stated in `CvInterp::Spline`'s doc, the ONE compiled copy). Non-causal
  splines REJECTED on the record: true Catmull-Rom needs `v[N+1]` and the natural cubic needs a
  global solve — both buy smoothness with a block of LATENCY, which would make `spline` time its
  wire differently from `linear`; PCHIP rejected too (its limiter bends parabolas near extrema,
  losing the exactness property; the clamp already provides the safety where it matters).
  Properties, each gated: **exact for quadratic-in-block-index sweeps**; **bit-identical to
  `linear` on collinear knots**; constant in, constant out (the coefficients sum to 1); and
  **`linear`'s arrival contract** — frame 0 IS the previous knot exactly, `cur` is reached at the
  next block boundary, so the declaration changes the shape of the ride, never its timing. f64
  evaluation with ONE rounding per frame (the mixer's cv-sum rule, WO-014 inc 6). **State:**
  `CvPlan` grew `prev2` beside `prev` and the wire's `range`; both history slots start at 0.0 —
  the same declared zero-history ramp `linear` has always had — so the first two blocks are a
  documented deterministic startup transient, gated. **Range discipline (decision 4):** the
  parabola carries momentum and can locally exceed the knot span (all interpolating splines do —
  the inertia IS the smoothness); the overshoot is HOST-made, so the spline branch clamps to the
  receiver's declared range (`CvRange::clamp_f64`, new), in words in all four copies of the rule.
  `hold`/`linear` deliberately do NOT clamp: they provably cannot exceed their knots, and an
  out-of-range frame there can only be a SOURCE bug, which must stay visible. **One signature,
  one copy (decision 5):** `expand` widened to `(prev2, prev, cur, range, dst)` — clamping in the
  executor instead would put the rule apart from the math it bounds; the api-snapshot pin moved on
  purpose with the reason at the pin. **The refusal retired; its philosophy is kept** — every
  other cv refusal (range, fan-in, delay-kind) stands untouched, and contract_v1's refusal gate
  was REPLACED in place by a behaviour gate. The G4 drift gate grew a pin that the matrix cell
  carries the curve's definition in the same words as the compiled copy. **`tests/cv_spline.rs`
  (8 gates):** the hand-computed parabola over a scripted dyadic sequence (every frame of every
  block `==` against independently simplified polynomials — the sequence visits the clamped
  overshoot, the momentum dip and the plain descent); the constant wire flat bit-exactly;
  spline ≡ linear bit-for-bit on a ramp with a hand-computed anchor so "equal" cannot mean
  "equally wrong"; a `v[N] = N²/64` sweep reproduced exactly at fractional frames from block 2
  on; frame 0 == the previous knot over an LCG-seeded sequence (the house seed 0xA17E); the
  startup transient hand-computed AND a rebuild byte-identical; both polarities' clamps at exactly
  ±1.0 with the un-clamped interiors keeping their hand-computed values; zero allocations over
  1 000 blocks. **Nothing else moved, by the shape of the diff (decision 8):** the hold/linear
  branches are byte-identical, no shipped module declares `spline` (grep-verified), no demo patch
  contains a block→audio cv edge, and the stress/determinism generators never synthesise an
  interp. **752 tests** (was 742; +8 cv_spline, +2 port.rs units, contract_v1 replaced 1:1),
  every pre-existing golden re-verified unchanged (`ba577186c988db21`, drum-demo
  `914d9063ce9d8a0f`, the three pinned exec renders at their `--seconds` — `1621e1f65b1b64e1`,
  `f2303f13aa0cf299`, `53de3b1f3f40e3c9`), stress `b42068ec7b206789` with identical counters
  (10 001 blocks · 7 203 swaps · 2 797 refused) in debug AND release, selftest 9/9, `ui --audit`
  PASS 25 smokes (the canvas never saw the refusal — interp lives in the executor, not in
  `connect_cv`), `modules --strict` 17/17, `module_docs --check` 17, fmt clean, clippy clean in
  default / bootstrap-audio / ui / native ui-window-gles / all six runnable MSVC cells (the two
  documented `windows`-crate OOM classes stay out), 5 python gates + `sync_check --self-test` +
  `log_check --self-test` 25/25. **Defect #88 (docs-only, fixed in passing):** the author guide's
  §11 still called `util/mixer` "Phase 1, not yet built" — stale since WO-014 inc 6; the bullet
  had to be rewritten for `spline` anyway. Sealed as **`sync wo008-inc6`** — WAITING, stacked on
  `sync-wo014-inc6.zip`. Parked and declared: the non-causal splines (a latency CONTRACT decision,
  not an implementation detail — a new word with its own gate if ever taken, never a quiet change
  to this one), and any shipped module ADOPTING `spline` (a module change moves goldens — its own
  increment, on purpose, with the audible A/B recorded).
- **WO-014 increment 6 is BUILT and sandbox-green — `util/mixer` 0.2.0, the cv merge side:** the
  sandbox track's next buildable item (item 6 — the live HAL through `SharedEngine` — stays gated
  on WO-006's device acceptance; the plan of record is `WO014-INC6-PLAN.md`). The compat-matrix's
  cv-fan-in cell always named `util/mixer` as the explicit merge; the module now IS one on both
  sides: four block-rate unipolar cv inputs (`cv-0..3`, `cv_reduce = "mean"` DECLARED per G3),
  per-input gains (params 20..23, **appended LAST** — every v0.1.0 snapshot index and every audio
  golden keeps its bits; short snapshots read 0.0 past their end, so batch-3's 20-value tests are
  unchanged BY CONSTRUCTION), one `cv-out` summing in f64 and clamped to the declared range (the
  module's own documented rule). Identity default: an untouched cv side is a bit-exact wire from
  `cv-0`. `BlockStatus` stays the AUDIO contract — all-unconnected audio ⇒ `Silenced` + exact
  zeros while the cv side merges (a cv-only mixer is a legitimate citizen, pinned). Wrong-rate
  presentation ⇒ `Failed` (the env/ad precedent). **The executor's fan-in refusal is RE-WORDED,
  not removed** (no implicit summing, ever) and now names the built remedy: each source to its own
  `cv-N`, destination fed from `cv-out` — `contract_v1`'s "names the merge module" pin passes on
  the new text. `Adapter::Merge.first_phase` stays Phase 1 ON PURPOSE (the module exists; the
  one-tap INSERT offer waits — a merge is a three-wire re-patch, not the inline pair shape the
  mechanism fits; decision 8 of the plan). **`tests/mixer_cv.rs` (9 gates):** bit-exact identity
  wire, hand-computed gain sum, exact 1.0 clamp, the declared mean reduce checked against the
  source's OWN published buffer in the same block, the audio side re-proven at 24 values, status
  decoupling, the remedy-naming refusal, zero allocations over 1 000 blocks, and the
  snapshot-order pin (a future insert-instead-of-append fails here first, in words). **A gate
  caught a real fragility mid-increment:** the inc-5 inspector-scroll smoke failed when the
  mixer's param count moved 20 → 24 — its fixed drag had been tuned to the old `max_scroll`; the
  smoke now asserts the CONTRACT (something hidden becomes visible AND touchable), not one
  module's arithmetic. **742 tests** (was 733), every pre-existing golden re-verified unchanged
  (incl. drum-demo through the mixer, the three pinned exec renders, stress `b42068ec7b206789`
  debug AND release), selftest 9/9, `ui --audit` PASS 25 smokes, `modules --strict` 17/17, docs
  regenerated (17) + `--check`, all runnable clippy cells clean (MSVC×8 now). Sealed as
  **`sync wo014-inc6`** — WAITING for the device. Parked and declared: the bipolar cv side
  (waits `util/range`), a 4×4 cv matrix (waits MAX_PARAMS), the canvas insert-offer for merges.
- **WO-013 increment 5 is BUILT and sandbox-green — the "Else column" is empty:** the four items
  increment 4 declared for its next pass all shipped, and nothing else (risk R1). **Rename text
  entry:** NEW `sparq-ui::canvas::entry` (`TextEntry` — end-caret, printables-only, 32-char cap =
  the node header's budget; `RenameState` — the sheet geometry on the browser's clamping rule),
  the RENAME menu row (first on the node menu; the one test that hard-coded DUPLICATE at row 0 now
  finds rows BY ACTION), commit through the increment-1 `Op::Rename` (undoable; EMPTY commits
  `None` = the module default; an unchanged buffer is a stated no-op with no history entry —
  catching the case `op_rename`'s equality cannot), cancel stated in words on Escape / outside tap
  / drag / long-press. The shell's keyboard path is ONE feed now: `read_keys` parses egui events
  into a normalised `KeyBatch` the rename sheet and the browser query both consume — the shared
  surface the provisional feed was declared to be waiting for. **Inspector scrolling:**
  `compute_at` with the offset clamped IN the layout, a fixed header the rows slide under, one
  visibility rule for draw/hit-test/audit (`row_visible`/`visible_rows`: a bottom sliver stays
  touchable, a row under the header is not), a hairline `thumb()` only when the panel scrolls
  (deliberately not an audit element — the gesture is the input). The gesture rides the TWO-finger
  pan (one finger on a row is a slider edit): **`GestureIntent::Pan` grew `center`** (the centroid
  the recogniser already tracked; `Zoom`'s precedent) — centre in the panel ⇒ scroll, elsewhere ⇒
  camera, exactly as before; a pinch over the panel is DECLINED so the canvas behind never moves,
  which also keeps the recogniser's sequential-contact span wobble (measured: reciprocal
  1.667/0.6 zoom factors on a straight two-finger drag) off the camera. Scroll is transient view
  state, reset on selection change; the panel speaks only at its ends. **Per-cv wire levels**
  (inc 4's DECLARED LIMIT, retired): `NodeLevels` grew per-port entries, `wire_level` prefers the
  source PORT's level and falls back to the node's fold (audio semantics untouched — pinned by a
  test), and `bridge::node_levels` reads each cv output's real published value (`node_cv_block` /
  `node_cv_audio`, magnitude rule) — **the executor was not touched**, so the stress hash
  `b42068ec7b206789` stands in debug AND release. Two bridge gates: the rms→svf thesis rig lights
  the cv wire at the metered rms while the SAME node's folded meter stays at rest, and dropping
  the sine's amp takes the level down. Per-port AUDIO meters + the ring extension stay parked for
  the live-HAL increment. **The LOD pass:** Dot wires are hairlines and the SUM word is
  suppressed there (declared); Simplified is truly text-free; states ride the look-board's
  pattern-first language at every level — hatch = bypassed (the mockup's `hatch8`), dashed border
  = muted, double border = locked, header chip / accent ring = master; at Dot, hollow = bypassed,
  dimmed = muted, concentric ring = locked. Words ride on top at Full only — the mockup's own
  redundant pairing. The side-by-side human review stays device-track (test006 step H). **Five
  new audit smokes (25 total)**; the cv smoke reads the bridge DIRECTLY on purpose — a second
  menu-driven render would overwrite `canvas-render.wav`, the file test006's step [05] hashes as
  the manifest-defaults baseline (smoke 13 owns the menu path). **733 tests** (was 713), every
  pre-existing golden re-verified unchanged, `modules --strict` 17/17, all runnable clippy cells
  clean (MSVC×ui-window and MSVC×bootstrap-audio remain the documented `windows`-crate OOM
  class). Sealed as **`sync wo013-inc5`** — APPLIED on the device 2026-09-27. Plan of record:
  `WO013-INC5-PLAN.md`.
- **WO-014 increment 5 is BUILT and sandbox-green — the module set is COMPLETE at seventeen:**
  the last three modules shipped on the contracts they waited for. **`ana/tap`** (the generic
  signal tap for displays): audio-rate bipolar `wave` (the mono monitor mix) + block-rate unipolar
  `peak`/`rms`; its wave golden **`3f325d4f99ca2a01` CROSS-VALIDATES against the `syn/sine` 1 s
  golden** — the mono mix of a mono-fanned sine is the sine itself bit for bit, so the tap colours
  nothing (two mechanisms, one bit pattern); `gain` scales the WAVE only, peak/rms report the TRUE
  signal. **`dsp/scope`** (the first real visual module): the set's only module whose `process` is
  a deliberate **NO-OP** — zero-audio-thread-cost is architecture, not optimisation, and the proof
  is a patch-level golden: adding a tap+scope to a render leaves the master output BIT-IDENTICAL,
  so the scope-rig golden **EQUALS the out/main golden `75bc7f2f18cac9d5`**; two audio-rate bipolar
  cv inputs (`x`, `y`) are the display BINDING the UI resolves to source taps. **`out/main`** (the
  master output with the metering hook): a unity-**BIT-EXACT** pass-through with trim + a hard mute
  writing exact zeros; because the executor meters every node, its peak/rms ARE the master meters;
  it has an audio OUTPUT because the executor renders the master node's first audio output (a pure
  sink would render silence). **The analysis-payload contract they waited on** shipped too:
  `sparq-audio::engine::AnalysisUpdate` extends ADR-009 d8's publication from meters to WAVEFORMS —
  `AudioEngine::publish_analysis` pushes every audio-rate `cv` output onto a bounded `SpscRing`
  per block, `SharedEngine::read_analysis` drains it control-side, `Executor::with_audio_rate_cv_out`
  is the allocation-free hook (`tests/analysis_pub.rs`: the waveform crosses, an absent reader is
  counted-not-queued, the audio thread allocates nothing). **The LFO beat-rate DECISION** the WO
  asked for before building: **module-side bpm derivation, NOT a per-frame tick view** — recorded
  with its reasoning in LATER.md (a contract change to buy sample-accurate tempo almost no module
  needs, vs reading the block-start tick the executor already carries; one block = 1.3 ms of lag on
  a tempo EDIT) — **and then SHIPPED the same session**: `mod/lfo` 0.2.0 (`sync-mode` + `division`)
  and `util/delay` 0.2.0 (`tempo-sync` + `division`, riding the once-unreachable
  `DelayLine::set_tempo_sync`) both derive bpm from the block-start tick via a shared
  `TempoFollower`, fall back to their free-running parameter with no transport, and are additive
  (defaults byte-identical, which the unchanged goldens prove). `tests/tempo_sync.rs` (7 gates):
  the derived rate is the transport's, it tracks a tempo change, the tempo-synced delay renders
  BIT-IDENTICAL to a hand-timed 250 ms, beat-locked golden `db4013f41d1fa678`, zero allocs.
  **713 tests** (was 682), every pre-existing golden re-verified unchanged (stress
  hash `b42068ec7b206789` in debug AND release), `modules --strict` **17/17**, docs regenerated
  (17), all runnable clippy cells clean. Sealed in **`sync wo014-inc5+wo013-inc4`**.
- **WO-013 increment 4 is BUILT and sandbox-green — live wire levels, the "signature sparq image":**
  the park note said wires drew at rest until the executor taps existed; they do now (WO-008's
  `Executor::meter` + the cross-thread `SharedEngine::read_meters`), so the painter half shipped —
  **and does not fake them**. **`sparq-ui::canvas::levels`** (toolkit-independent, computed not
  drawn): `NodeLevels` (per-node 0..1, clamped, NaN-to-rest, deterministic) + `wire_level` (a wire
  carries its SOURCE node's level), unit-tested. **`bridge::node_levels`** renders a short preview
  and reads each node's PEAK meter, mapping kernel nodes back to canvas nodes — the ONLY source of
  a wire level, never a value invented from canvas data; two bridge tests prove the levels are real
  AND follow the signal (turn the gain down, the level falls). **The painter** (`canvas_ui.rs`)
  modulates each wire from its class colour toward its class glow by level plus a soft under-glow —
  both endpoint colours are tokens, the blend invents nothing. `CanvasState.levels` is a transient
  (non-undoable) field the shell refreshes from the bridge after RENDER WAV; empty until the first
  render, so wires draw exactly as before until then. **Audit smoke 20** proves RENDER WAV populates
  live levels from real meters. **The master handover** (coupled to `out/main`): `resolve_master`
  now prefers a wired `out/main` node over the default highest-id-terminus rule (an explicit SET
  MASTER still wins; an unwired out/main does not count), 3 new tests, `OUT_MAIN_ID` a named
  constant so the rule and the id cannot drift. **DECLARED LIMIT (not faked):** a node's meter folds
  its AUDIO outputs, so a cv wire out of a cv-only source (lfo/env/rms) reads at rest — the
  animation tracks AUDIO signal flow; per-port/per-cv meters are the LATER.md item that would light
  cv wires from their own values. **NOT in this increment** (the task list's "Else" column, declared
  for the next pass): rename text entry, inspector scrolling, LOD visual iteration vs
  `design-mode.svg`. Sealed in **`sync wo014-inc5+wo013-inc4`**.
- **WO-008 increment 5 is BUILT and sandbox-green — the cross-thread hot-swap primitive:** the
  task list's item 3, "the only remaining increment with a proofs burden this heavy", is done.
  **`sparq-kernel::sync::hotswap`** (allowlist entry 6, shipped in `sync/` with the auditor's
  alias like entry 4's): `HotSwap::split` → `HotSwapControl` (stage complete boxed payloads,
  reclaim retired ones, counters) + `HotSwapAudio` (`boundary(inherit)` once per block — d3's
  LITERAL pointer swap, one `AtomicPtr::swap` per side). **Epoch retirement:** the boundary
  publishes the outgoing payload into one of 8 epoch-tagged slots and stores `epoch+1` Release;
  the controller may re-box a retirement only when its Acquire-loaded epoch exceeds the tag —
  the grace period is mechanical, and the drop lands control-side, so module deactivation never
  runs in a device callback. Slot exhaustion DEFERS the swap (counted) — the audio thread's
  every fallback is "keep rendering the live patch". **The contract grew one bound:
  `Module: Send`** (built control-side, rendered audio-side, retired control-side; additive,
  all 41 implementors satisfy it automatically, pinned in api_snapshot; `Sync` deliberately not
  required). **The cross-thread engine** (`sparq-audio::engine`): `LivePatch {executor, master}`
  payloads, `SharedEngine` (stage / `set_params` / `set_musical_position` / reclaim /
  `read_meters` / stats) + `AudioEngine` (per block: boundary swap with `inherit_runtime` at
  the swap point → `EngineCmd` ring drain → render → meter publication). **Decision 8's first
  consumer:** per-node `MeterUpdate`s (peak/rms/status + block clock) ride an `SpscRing` out;
  `Copy` commands ride one in; both refuse-and-count, never block, never grow. The
  single-owner `Engine` stays untouched — the determinism harness keeps its vehicle and the
  stress hash `b42068ec7b206789` its meaning (the `World::candidate_mutation` extraction that
  lets the cross-thread stress reuse the seeded schedule preserved the RNG draw order exactly —
  verified in debug AND release). **The proofs:** 9 kernel tests (epoch gate white-boxed in its
  three states, deferral without drops, teardown hygiene both orders, a paced 5 000-payload
  two-thread exchange with tearing checks; `cfg!(miri)` scales it to 200) + 7 audio tests
  (`tests/cross_thread.rs`) + **the acceptance: 10 000 seeded mutations staged from a control
  thread while the audio thread plays** — 7 400 swaps, 2 600 refusals that left no trace,
  ~43 000 rendered blocks, ZERO failed blocks, ZERO audio-thread allocations under the counting
  allocator, `deferred == superseded == 0`, retirements == swaps == reclaimed, clock never
  regressing; no golden hash ON PURPOSE (which block a mutation lands on is a scheduling fact —
  a golden over scheduling is a flake generator). **selftest gate 9** (64 paced swaps across two
  threads in-process) gives SATURN MSVC-side evidence; CI's Miri cell widened `sync::rings` →
  `sync::` (the sandbox cannot run Miri — nightly sysroot build OOM-killed at 1 GB, tried
  twice; environment, same class as MSVC×ui-window). **682 tests** (was 665), every
  pre-existing golden re-verified unchanged, `selftest --golden` 9/9, audit PASS 19 smokes,
  `modules --strict` 14/14, all clippy cells clean. **Defect #85** logged: the exec evidence
  hashes were recorded without their durations (the CLI default renders 5 s; the goldens are
  the 2.8 s/1 s/2 s renders) — the standing order now carries explicit `--seconds`. Sealed as
  **`sync wo008-inc5`**. Unblocked and waiting: `ana/tap` + `dsp/scope` (WO-014 inc 5), WO-013's
  live wire levels (inc 4), and — after WO-006's exclusive acceptance — routing the live HAL
  stream through `SharedEngine`. **Device note:** the pristine clone FAILED `sync_check
  --quiet` against its own committed stamp (defect #86 — the git history rebuild lost 230B of
  `sparq-module-api/src/lib.rs` and sparq-music from `sync_check.py`'s roots); the bundle
  carries both files as declared drift repairs, and the run sheet starts with two `copy`
  commands that save SATURN's sealed versions to `logs\` before overwriting.
- **WO-014 increment 4 is BUILT and sandbox-green — the clocks' first consumers:** the module
  set reaches **fourteen** with the two `mod`-family modules defect #84's taxonomy fix existed
  for. **`mod/lfo`**: four shapes mapped into 0..1, audio-rate UNIPOLAR cv out (the range
  decision is recorded in the manifest header: the matrix refuses bipolar→unipolar rather than
  rescale, and `util/range` is Phase 1 — a bipolar port today could not connect to a single
  shipped consumer), event phase-reset at the exact sample, 8-byte phase state. Gates use a
  **binary-exact rate** (48000/2^15 Hz) so quarter-cycle values are EQUALITIES and the wrap
  lands on frame 32768 to the bit. **`mod/clk-div`**: the first event-PROCESSING module —
  divide counts from the first input, multiply schedules sub-triggers across the MEASURED
  interval from a bounded 32-slot internal schedule (exact samples in later blocks), the
  probability gate is a seeded xorshift draw (same seed ⇒ same decisions, tested both ways;
  p=0/p=1 exact). The acceptance-shaped gates: **lfo→svf cutoff renders BIT-IDENTICAL to a
  hand-driven reference** over 200 blocks (the contract-v1 pattern reused; sweep ends at
  289.445 Hz and rising), and **host 16ths → ÷4 → membrane** lands kicks on exactly frames
  0/24576/49152/73728 with chain golden **`914d9063ce9d8a0f`**. Zero allocations: both modules
  (5 000 calls each) and the divided-drum chain (1 000 blocks, in-loop host pushes). **665
  tests** (was 655), every pre-existing golden unchanged, stress hash unchanged, selftest 8/8,
  audit PASS 19 smokes (browser meets "LFO"/"Clock Divider"; the "exactly one gain" smoke still
  passes), `modules --strict` **14/14**, docs regenerated (14), all clippy cells clean.
  Sealed as **`sync wo014-inc4`**.
- **WO-009 increment 1 is BUILT and sandbox-green — clocks + transport v0:** the kernel `Clock`
  is now ADR-006 rule 2's piecewise map (anchored segments, linear bpm ramps — derivative
  continuous by INHERITANCE, exact integer bisection for `sample_at_tick`, the constant-segment
  fast path bit-identical to v0 so the phase-b goldens could not move); **`sparq-music`** is the
  planned crate's first contents (`ClockBroker`: position + wall-drift estimator, backwards
  readings refused-and-counted; `Transport`: freeze-on-stop, tempo-as-segments, block-granular
  loop fold, tap tempo, the tick-scheduled queue, bar/beat emission, `advance_block` zero-alloc
  measured over 10 000 blocks into a bounded 64-slot collector). The executor gained
  `set_musical_position` — **the transport computes, the executor carries** — and a never-set
  door renders the static `tick = 0` every golden knows (stress hash `b42068ec7b206789` unmoved).
  The ±0-sample acceptance ran 1 000 seeded ticks — constant tempo AND across a glide + a step —
  against an **independent implementation of the segment integral inside the test**. The
  cross-check: `exec --patch drum-demo` is transport-driven (beats AND bars, live tick) and
  hashes to `f2303f13aa0cf299` — **the identical golden** WO-014 inc 3's block counter produced.
  Two mechanisms, one bit pattern. One finding caught by measurement pre-ship: proportional beat
  placement inside loops put boundary beats one sample early (23999 vs 24000) — beats now ride
  the absolute map grid; the transport header records it. **655 tests** (was 627), all goldens
  unchanged, ADR-006 addendum scores the four acceptance boxes honestly (three met in sandbox;
  the drift box's 30-min hardware half stays device-track). Sealed `sync wo009-inc1`.
- **WO-014 increment 3 is BUILT and sandbox-green — contract v1's first consumers:** the module
  set reaches **twelve** with the three modules the contract unblocked — **`syn/membrane`** (the
  Phase-B kick topology promoted to a trigger-driven voice; a trigger at sample 37 leaves frames
  0..36 at EXACT zeros and lands the hit in its own block — the WO's sample-accurate stress,
  measured), **`env/ad`** (event in → audio-rate cv out and **no audio ports at all**; loop and
  gate-off semantics per its manifest, the loop cycle measured against the wrapped `AdEnv`'s
  epsilon-arrival decay rather than the parameter faces), **`util/mixer`** (4×4 stereo matrix,
  20 of 32 snapshot params — which is WHY 4×4, declared; identity default is a bit-exact wire;
  the explicit merge the fan-in rules require; `Silenced` when all-unconnected). Plus
  **`sparq exec --patch drum-demo`**: host triggers at 120 BPM → membrane → mixer(trim 0.6),
  golden `f2303f13aa0cf299` with the four kicks asserted on EXACTLY frames 0/24000/48000/72000
  (debug == release, two builds bit-identical). **Defect #84** fixed table-first: `classification.top`'s
  closed domain was missing `env`/`mod` in BOTH copies while Appendix B's ids and WO-014's own
  artefact paths name them — `env/ad` could not declare its own top; the table moved first, then
  `TOPS` (16 → 18), then the field table gained its first code consumer as a drift pin. The
  executor's cv-fan-in refusal was re-worded (it named `util/mixer` as "not in this build" — the
  audio side now ships; the cv side is LATER.md). **627 tests** (was 614), every pre-existing
  golden unchanged, stress hash unchanged, zero allocations counted across 15 000 new-module
  `process` calls and 1000 drum-demo blocks with in-loop host pushes, selftest 8/8, audit PASS
  19 smokes (three new names checked against the fuzzy scorer), `modules --strict` **12/12**,
  docs regenerated (12), all clippy cells clean. Sealed as **`sync wo014-inc3`**.
- **WO-008 increment 4 is BUILT and sandbox-green — CONTRACT V1, the "unblocking increment":**
  the multi-port `AudioCtx` (audio ≤ 8 ports/class + `take_*` views for multi-output modules,
  `cv` block/audio-rate ports at the RECEIVER's declared rate, `event` ports pre-sorted with G5's
  stable tie-break and bounded counted-overflow sinks), the executor's cv/event wiring (all six
  `cv_reduce` policies measured against hand-computed values, `cv_interp` hold/linear with
  `spline` REFUSED in words, range-mismatch + fan-in refusals through `connect_cv` at `Phase::Zero`
  naming the converter AND its phase, `event_kinds` subset checks, the control-side
  `push_host_event` door that WO-009 will feed), data/gpu/atom still refusing per type in words.
  **`ana/rms` publishes on its declared cv port** (the output[0] convention is deleted — exec's
  tap line reads the same `0.16621882` through `node_cv_block`), **`flt/svf` 0.2.0** grows the
  `cutoff-mod` cv input + `mod` param (additive; at defaults bit-identical to 0.1.0). The
  **WO-014 rms→filter acceptance is proven with arithmetic**: the cv-wired filter renders
  **bit-identical** to a hand-driven reference over 200 blocks, and `sparq exec --patch mod-demo`
  is the audible artefact (golden `1621e1f65b1b64e1`, debug == release). The **compat-matrix
  drift gate** landed (the WO-007 debt, answered honestly — see below) and caught defects #80–#82
  on first run; `toml.rs` learned nested `[[a.b]]` arrays (the matrix had never been parsed by
  the crate that ships the parser). **614 tests** (was 559), **ALL pre-existing goldens
  UNCHANGED** (`ba577186c988db21` / `0f5c3e86c7f117a9` / `53de3b1f3f40e3c9` / `3f325d4f99ca2a01`
  / six batch-2 goldens / chain `9170415cd3852736`) and the **stress hash `b42068ec7b206789`
  UNCHANGED** (10 001 blocks · 7 203 swaps · 2 797 refusals · 0 audio-path allocations), selftest
  8/8, `ui --audit` PASS 19 smokes, `modules --strict` 9/9, clippy clean in default /
  bootstrap-audio / ui / native ui-window-gles / MSVC×5, 5 Python gates + module docs clean.
  Sealed as **`sync wo008-inc4`**, stamp **`src 86f/1679258B`** (134-file covered set verified,
  self-test re-proven failable 11/11; `log_check --self-test` 25/25).
- **The compat-matrix mirror debt, re-worded not hidden:** the gate pins the table and `port.rs`
  together (vocabularies, verdicts, adapter ids, per-type case counts, a representative outcome
  per audio/cv `when` cell). Full deletion stays open WITH A NAMED REASON: several `when` cells
  are prose ("fan-out: one cv output -> many cv inputs") — shape rules, not pair rules — so "read
  at discovery" needs the ratified table restructured into machine predicates first (a
  review-packet change; `[meta] reviewed = false` is still pending). Recorded in the ADR-005
  addendum. Defect **#82** (mono→ambisonics is "compatible fan-out" in BOTH copies — consistent,
  musically questionable) is pinned by a test and flagged for that review.
- **WO-014 increment 2 is BUILT and sandbox-green** — the six AUDIO-domain modules
  (`syn/noise` seeded+`reset`, `syn/polyblep` (stable id, bandlimited-additive impl per #12 —
  the manifest says so in words), `flt/svf` five modes, `util/delay` with the set's first
  **parametric latency** (`"param:time"`), `fx/bitcrush` seeded per-channel dither,
  `util/panner` two laws): **9 first-party modules total**, `modules --strict` 9/9, six module
  goldens + a five-module chain golden checked in, per-module 5 000-call zero-alloc gates, the
  **polyblep aliasing acceptance measured through the registry build: −166.8 dB vs the −60 dB
  bar** (printed by the test every run), the delay echo landing at exactly its declared sample.
  **`tools/module_docs.py`** ships the generated-docs acceptance (9 docs, `--check` wired into
  justfile + ci.yml + gates.bat, proven failable pre-ship). Sealed as **`sync wo014-inc2`**
  (applied on SATURN). **Of the eight modules that did not ship, THREE ARE NOW UNBLOCKED by
  contract v1** (`syn/membrane`, `env/ad`, `util/mixer` — event/cv/multi-port payloads travel;
  `tests/contract_v1.rs`'s probe modules are the existence proof); `mod/lfo`/`mod/clk-div` still
  wait on WO-009's clocks, `ana/tap`/`dsp/scope` on the ring publication, `out/main` on the
  canvas master-handover.
- **WO-008 increment 3 is BUILT and sandbox-green** (tasks 4–7): the boundary-swap `Engine`, the
  **10 000-mutation stress** (`tests/mutation_stress.rs`), kernel **per-path latency** + the
  **raw/compensated switch**, the **watchdog** (3 consecutive overruns ⇒ auto-bypass + bounded
  journal), and the **determinism harness** (`sparq_audio::determinism` — it caught a real
  HashMap-iteration-order nondeterminism on day one). Sealed as **`sync wo008-inc3`** (applied).
- **WO-006 increment 1.3 is BUILT and sandbox-green** (the defect-#79 exclusive device-period
  fix): `sparq-kernel::hal::period` (pure ladder, 7 Linux unit tests incl. the exact test004
  device shape), `open_exclusive` executes it with a fresh client + alignment two-step per
  candidate and a full period probe table on refusal, pump unchanged. Sealed as
  **`sync wo006-inc13`** (applied). Device contract = `scripts\test004.bat` +
  `WO006-INC13-RUN-SHEET.md`.
- **WO-013 increment 3 is BUILT and sandbox-green** (browser + inspector + wire re-patch).
  Sealed as **`sync wo013-inc3`** (applied). Device contract: `scripts\test006.bat` +
  `WO013-INC3-RUN-SHEET.md`.
- MSVC × `ui-window` cell: **still unrunnable anywhere** — the `windows` crate's rustc is
  OOM-killed at 1 GB (environment, not code; CI has no such cell either). The native
  `ui-window`/gles cell builds clean at `-j 1` with dev debuginfo off.
- **Miri is CI-only** (observed 2026-09-27): `cargo +nightly miri` needs a custom sysroot whose
  `core` build is OOM-killed at 1 GB — tried twice. The `sanitizers` CI job (continue-on-error)
  runs `sync::` under `-Zmiri-strict-provenance`; the hotswap suite scales itself down via
  `cfg!(miri)` so the cell stays usable. Nightly installs with `rustup toolchain install nightly
  --profile minimal --component miri` if a future sandbox has the memory.

## 🔴 THE BLOCKER (device track — waiting on SATURN, not on code)

**WO-006 exclusive acceptance = the `test004` re-run, attempt 4** (every sealed bundle through
`sync-wo006-inc15.zip` is APPLIED on the device — operator-reported 2026-09-29; the HAL fix is
on the machine, attempt 4's digest is the missing piece. `sync-wo008-inc7.zip` rides along —
it changes no test004 contract, only the gates' numbers). **This blocker keeps the ADR-008
exit, the stage-path proof and the #92 diagnosis — but since the operator's session-6-close
pivot it no longer gates the UX track: live canvas audio proceeds in parallel on the proven
shared backend (header + sandbox item 6), and attempt 4 is a ~15-minute device run whenever the
machine is free, not a dependency of the sandbox's next build.** History:
**attempt 1** (2026-09-24) passed the caps checkpoint (#77 verified on hardware) and was refused
at every rung with `AUDCLNT_E_INVALID_DEVICE_PERIOD` — the open asked the 64-fr block (666.7 µs
@ 96 kHz) against a 10 ms engine (**defect #79**, fixed in inc 1.3). **Attempt 2** (2026-09-28
morning) got further: exclusive OPENED — `i24-in-32 (converting)` @ 96 kHz — but at **288 fr
(3.00 ms)**, the driver's reported minimum/granularity, and stalled: 159 late wakes / 10 s, half
throughput, drift −49 %, no clean tone (**defect #89** — `Initialize` accepts, runtime refuses;
fixed in inc 1.4 by the default-first ladder). **Attempt 3** (2026-09-28 evening) proved the
inc-1.4 fix ON HARDWARE — the open landed exactly on the predicted `device period 960 fr
(10.00 ms)` shape, no shrink note — and the stall stayed, unchanged in wall shape (~16 stalls/s,
~40 ms each) at the new period: **the stall is period-independent (defect #92)**, and the exact
identity delivered = wakes × buffer on both attempts says why it costs half the audio:
event-driven exclusive cannot legally bank more than one period (`hnsPeriodicity` ≡
`hnsBufferDuration` under `EVENTCALLBACK`), so every swallowed cadence is lost. Attempt 3 also
exposed **defect #91**: the `96000 Hz` was never play's ask — the exclusive envelope had been
sieved through the shared probe (lone `exclusive 96000`) and the adjustment bent the standing
48 kHz request UP into the one rate the exclusive path misbehaves at. Inc 1.5 decouples the
probes, ships #76's delivered-vs-wall drift as the instrument, and sends attempt 4 to **48 kHz**.
On attempt 4: [03] must list a MULTI-rate exclusive envelope including 48000 (a lone 96000 means
the driver genuinely refuses 48 kHz exclusive — a finding; send the caps block); [04]'s open line
should read `48000 Hz · 2 ch · i24-in-32 (converting) · device period 480 fr (10.00 ms) · sparq
block 64 fr` with NO `adjusted` line above it (and no `NEGOTIATED` line — its absence is the
good case), drift within a few hundred ppm of 0, 0 late wakes, `play: PASS`, clean tone. **If
the triple returns at 48 kHz, #92 is rate-independent** — the digest now says so in measurements
(drift is delivered-vs-wall truth; suspect drift fails the verdict) — and the named next lever
is push-mode exclusive (LATER.md), its own increment. When [04]+[05] pass and the **2 h
zero-xrun exclusive soak at the rate [04] proves** finishes clean — the PROPOSED amendment of
the WO's "96 kHz" wording, run sheet §2, ratified or struck by the next session against the
attempt-4 evidence; the [07] prompt offers 96000 as the stretch — : WO-006 acceptance closes →
ADR-008 exit fires → **next sandbox increment: delete the cpal bootstrap, HAL becomes `play`'s
default** (that deletion is gated on this device run — do not do it early).

## Next session's task list, in order

**Device track (SATURN — one bundle WAITING to apply, then the remaining work is EVIDENCE):**
1. ~~APPLY `sync-wo006-inc15.zip`~~ **DONE (operator-reported 2026-09-29). APPLY
   `sync-wo008-inc7.zip` (17 entries), THEN `sync-wo012-inc2.zip` (20), THEN
   `sync-wo013-inc6.zip` (16)** — stacked in that order; none changes a device contract except
   the gates' numbers. Then **run the device gates
   and send the DIGESTS back** — every test script ends by making `NAME-digest.log` beside its
   log (compact copies: verdict lines byte-identical, only the periodic-status / cargo /
   duplicate repetition collapses; `scripts\digest.bat` does older logs on demand). Run
   `scripts\test004.bat` (the exclusive acceptance, **attempt 4 at 48 kHz** — pass shape, the
   PROPOSED acceptance-rate amendment and the failure-reading guide in
   `WO006-INC15-RUN-SHEET.md`; the 🔴 blocker) and `scripts\test006.bat` (the window session;
   step [07] is the mechanical claim: slider edit must change the canvas-render.wav hash — after
   RENDER WAV the sine/gain wires light with live levels — **the steps F–I: the rename sheet
   under real keys, the inspector two-finger scroll on a Mixer, the LOD walk vs
   `design-mode.svg` (write what differs into the H answer), the cv wire lighting from its own
   value — and the NEW steps J–K: PLAY audible on wasapi-shared with the wires animating WHILE
   it plays, and a live slider edit heard without stopping (the optional unplug → the Removed
   line and an untouched canvas)**). `scripts\gates.bat` now expects **777 passed / 0 failed / 1 ignored**, **17**
   modules (the mixer row reads v0.2.0, 13 ports, 24 params), **selftest PASS (9 gates)** and
   **`ui --audit` PASS (34 smokes)** — with the stamp `log_check.py` in the bundle already knows
   (`src 94f/2155888B`; defect #83's fix made moving it a seal step) — and the stress evidence
   line inside the test output reads the MOVED baseline (`7bb06379bd6845e5` · 2 383 swaps ·
   7 617 refused), which is inc 7's recorded intent, not a regression. Wanted evidence, **with
   the pinned durations** (defect #85): `sparq exec --patch mod-demo --seconds 1`
   (`1621e1f65b1b64e1`), `sparq exec --patch drum-demo --seconds 2` (`f2303f13aa0cf299`),
   `sparq exec --patch demo --seconds 2.8` (`53de3b1f3f40e3c9`), and — when the HAL is free —
   the two WO-009 device boxes: a LISTENED tempo sweep and the 30-min drift run (**now on the
   new delivered-vs-wall metric — old drift logs are not comparable, #76**). No drift-repair
   copy step was needed (the pristine clone passes `sync_check --quiet`; defect #86 was repaired
   in the wo008-inc5 bundle). **Wanted back: `test004-digest.log`, `test006-digest.log` (with
   the F–I answers), `gates-digest.log`, `logs\ui-digest.log`** — the digests, not the full logs
   (the full ones stay on the device; if a diagnosis ever needs a collapsed line, the run sheet
   says which full file to send instead) — those close the device boxes the sandbox cannot.
   **Also wanted, in the next session's words: a ratify-or-strike decision on the
   acceptance-rate amendment (run sheet §2) against attempt 4's evidence.**
2. If test004's 2 h soak passes → **WO-006 acceptance closes** → sandbox increment: ADR-008 exit
   (delete the cpal bootstrap, HAL becomes `play`'s default) — **still gated here, do not do it
   early.** The live-HAL-through-`SharedEngine` routing that used to ride this gate was
   RE-SEQUENCED ahead of it by the operator's session-6-close pivot (header + sandbox item 6):
   it builds on the proven shared backend while exclusive acceptance stays pending. Then WO-006
   increment 2 backlog (ASIO, duplex, round-trip measurement, STA retry for the #75 property-store
   `E_ACCESSDENIED` — the clock-drift re-derivation per #76 SHIPPED in inc 1.5 and leaves this list).
3. The WO-008 **loaded soak** (200 modules, 30 min, zero xruns) — the WO's hardware acceptance,
   and since inc 5 also the paced-real-time half of allowlist entry 6's stress sentence.

**Sandbox track (buildable now, in value order):**
4. ~~**WO-013 increment 4** — live wire levels~~ **DONE (2026-09-27)** ~~and its "Else"
   column~~ **DONE TOO (2026-09-27, second session — WO-013 increment 5):** rename text entry (the
   shared `canvas::entry` surface + the unified modal key feed), inspector scrolling (the
   two-finger pan routed by its new `center`), per-cv wire levels (the bridge reads the executor's
   real cv publications; the executor untouched) and the LOD pass (Dot hairlines, text-free
   Simplified, the pattern-first state language). What remains is declared in LATER.md §WO-013:
   numeric param entry (one parse step from the same `TextEntry`), caret movement/IME, browser
   drag-scroll, per-port AUDIO meters + ring port ids and the continuous level refresh (both ride
   the live-HAL-through-`SharedEngine` increment), randomise (the seed tree), and the device
   side-by-side LOD review (test006 step H).
5. ~~**WO-014 increment 5** — the last three modules~~ **DONE (2026-09-27, this session): the
   first-party set is COMPLETE at seventeen** — `ana/tap`, `dsp/scope`, `out/main` shipped on the
   analysis-payload contract (`AnalysisUpdate` extends d8's ring from meters to waveforms), with the
   two new goldens (`75bc7f2f18cac9d5` out/main == scope rig; `3f325d4f99ca2a01` tap wave == sine)
   and the LFO beat-rate DECISION — **decided AND shipped**: `mod/lfo` 0.2.0 + `util/delay` 0.2.0
   derive bpm module-side from the block-start tick (`TempoFollower`), and the delay's
   `set_tempo_sync` path is now reached (`tests/tempo_sync.rs`, golden `db4013f41d1fa678`).
   **Remaining from inc 5:** only the <15 %-of-a-core benchmark (stage-machine acceptance — a
   sandbox CPU ratio does not transfer, so no sandbox number is recorded as evidence).
6. ~~**ACTIVE — THE NEXT SANDBOX BUILD: the live HAL stream through `SharedEngine` — live
   canvas audio + continuous meters**~~ **DONE (2026-09-29, seventh session) — shipped as
   WO-012 increment 2, plan of record `WO012-INC2-PLAN.md`, sealed `sync-wo012-inc2.zip`:**
   the `LiveSession` (two-phase open, negotiated-config executor, the callback owning the
   `AudioEngine`), the one op→sync door (params → command ring, zero re-stages; structural +
   master handovers → boundary re-stage; moves/renames inaudible), the bounded per-frame drain
   into `CanvasState.levels`, the ring port ids + block-rate cv analysis riding it, five live
   audit smokes on the manual null (30 total), test006 steps J–K. The research notes below are
   HISTORICAL — the plan superseded them, and its postscript records the one decision the code
   corrected (`stage` supersedes, never refuses). **The named next sandbox build: the SCOPE
   SCREEN** (the analysis ring publishes waveforms with nothing drawing them — the plan's
   declared follow-on — **DONE TOO, same session, as WO-013 increment 6, sealed
   `sync-wo013-inc6.zip`**), then the canvas badge for the bare-required node (item 8 — NOW the
   named next sandbox build). `play`-style HAL stream whose callback holds `AudioEngine`; the control side stages
   from the canvas; `CanvasState.levels` fills from `read_meters` PER FRAME instead of the
   offline preview render (the continuous live-levels half; the offline half shipped in WO-013
   inc 4, per-port meters + ring port ids ride this increment, the scope screen — the analysis
   ring already publishes waveforms with nothing drawing them — is the named follow-on).
   **Research notes from session 6 (facts verified in the tree; NO code written, NO decisions
   made beyond the two in the header — the plan of record comes first):**
   * What already exists: `sparq-audio::engine`'s `SharedEngine::new(executor, master) →
     (SharedEngine, AudioEngine)` split; `stage(executor, master) -> bool` (boundary hot-swap,
     refuse-and-count when full), `set_params(node, ParamSet)` / `set_musical_position` / `send`
     (Copy commands over the `EngineCmd` ring — param edits need NO re-stage), `reclaim()`
     (control-side retirement hygiene), `read_meters(&mut [MeterUpdate]) -> usize` +
     `read_analysis` (bounded drains, absent reader counted-not-queued); `AudioEngine::
     render_block(out)` per block (cmd drain → boundary swap with `inherit_runtime` → render →
     meter publication); proven by the two-thread 10 000-mutation acceptance (0 failed blocks,
     0 audio-thread allocations). `bridge::build_with_map` already yields the canvas→kernel node
     map `node_levels` consumes; `NodeLevels`/`wire_level` (per-port entries since WO-013 inc 5)
     and the painter's level blend are DONE — the live path swaps the SOURCE, not the drawing.
   * Sketch (NOT decisions — the plan must settle each): a `LiveSession` owned by the app-side
     shell { HAL `Box<dyn AudioStream>`, `SharedEngine`, the node map, bounded meter scratch };
     canvas structural ops → `build_with_map` → `stage`; param edits → `set_params` with the
     mapped kernel node; per frame → drain `read_meters` into `NodeLevels` via the stored map;
     periodic `reclaim()`; backend choice: null for headless/audit (hermetic — the audit must
     not need a device), `wasapi-shared` default on the device (a device picker is later);
     stream `Removed`/`Failed` → one honest shell-log line + session stops, canvas untouched
     (every audio-thread fallback stays "keep rendering the live patch", inc 5's design).
   * Open questions the plan must answer: where the transport (start/stop) lives in the shell
     chrome and its touch shape (input-model rules); what the chrome shows of the negotiated
     config/latency/xrun truth (the play path's honesty lines have a house shape); the audit
     smoke design — paced null (timing on a loaded CI runner?) vs manual-mode null the smoke
     pumps itself (deterministic; `AudioStream::pump` exists for exactly this); interaction with
     RENDER WAV (the offline preview path stays as-is — separate concern); whether undo/redo
     re-stages through the same op→sync door (ops apply to `CanvasState` first, then sync —
     likely free); levels refresh rate-limiting (once per frame, bounded drain sizes).
   * Hard constraints: goldens untouched (no render path moves); the bootstrap stays (ADR-008
     gated — header); stress baseline is now `7bb06379bd6845e5` · 2 383 · 7 617; test-count
     baseline 762 moves with whatever gates ship; `ui --audit` stays hermetic and its smoke
     count moves only with new smokes.
7. Studio-session evidence still open: real-finger canvas touch-test, DPI matrix walk
   (WO-012/WO-013), WO-001 hardware sheets, dispatch-bench C/D numbers for ADR-009.
8. Small, well-specified, declared in LATER.md: ~~mixer's cv merge side~~ **DONE (WO-014 inc
   6)**; ~~`cv_interp = "spline"`~~ **DONE (WO-008 inc 6)**; ~~host-side `required`-unconnected
   enforcement~~ **DONE (WO-008 inc 7, this session — the baseline moved on purpose and is
   recorded)**; **the canvas badge for the bare-required node** (new from inc 7: the refusal
   reaches the shell log at RENDER; drawing the warning at DROP time is a WO-013-side painter
   increment, parked beside the watchdog→UI hairline it would share machinery with — item 6
   shipped, so this is now the smallest well-specified item after the scope screen); the compat-matrix review packet (`reviewed = false`, defect #82's
   ordering question inside it); loop-relative beat phase for unaligned loop regions (WO-009's
   declared limit); from inc 5: per-port meters (would light cv wires + a scope's per-channel
   display), a multi-reader epoch (a second reader of the live patch), and narrowing analysis
   publication to declared analysis sources if a large rig's ring traffic ever wants it.

## Done (most recent first)

- [x] **WO-012 inc 4** (2026-09-30, eighth session) — **the device blocker + the operator's
      four asks**: the per-session MTA audio-control thread (request-reply protocol, bounded
      waits, health mirror, Drop stops + joins — the STA refusal cannot recur in any host),
      mouse usability (hover dropped at the adapter, right-click = `Context`, wheel = `Pan`,
      no modifiers), `out/main` stereo meter bars (`MeterUpdate.peak_l/peak_r`, green data
      fills, amber peak-holds on audio time, empty at rest), and the grouped class-striped
      112×56 palette tiles (all seventeen visible at 1920). Plan `WO012-INC4-PLAN.md`.
      **777 tests** (+2 session gates), **37 smokes** (+3), goldens bit-identical, selftest
      9/9, 17/17, every clippy cell (the disallowed-`Mutex` lint took a reasoned control-path
      allow; `checked_div` and a boxed command variant for the enum-size lint), 5 python gates,
      stamp `src 94f/2188595B`, log_check BASELINE moved. Sealed `sync-wo012-inc4.zip` — the
      only waiting bundle. Device acceptance = PLAY makes sound (test006 J).
- [x] **WO-012 inc 3** (2026-09-29, seventh session, third increment) — **mockup conformance,
      chrome**: the operator's "make it look like design-mode.svg", sliced in two (this is
      slice A; B = node displays + inspector plots). Plan `WO012-INC3-PLAN.md`; the rail
      ruling (glyph + micro-label) and the slice are the operator's decisions via the question
      tool. Shipped: glyph+word rail with groups, level tick and ADD door; top-bar logo,
      dividers, live diagnostics line and LIVE dot; the wire legend from the wires' own
      encoding table; the dock's MODULES card grid (real spawn door, overflow in words, four
      tabs honestly disabled); inspector port-dot strip + control-cyan block-thumb sliders;
      `shell::compute`'s column discipline (rail/inspector full height, dock in the canvas
      column); the demo patch spread; and `sparq ui --svg-out` — the headless vector screenshot
      the review protocol never had, with the first convergence sheet committed beside the
      mockups. 777 tests unchanged (chrome moves no audio), 34 smokes, matrix 0 violations at
      the new element counts, every golden bit-identical, stamp `src 94f/2155888B`. Deviations
      declared as mockup-review findings 9–14. Sealed `sync-wo012-inc3.zip` — WAITING, stacked
      on inc6/inc2/inc7.
- [x] **WO-013 inc 6** (2026-09-29, seventh session, second increment) — **the scope screen**:
      `dsp/scope` draws from the analysis ring (plan `WO013-INC6-PLAN.md`, twelve decisions +
      the D3′ correction the smoke caught: the session is the traces' SINGLE SOURCE — the
      canvas-field swap alternated sets and blanked frames; the painter reads through a `draw`
      parameter, empty at rest). `sparq-ui::canvas::scope`: rolling traces sized timebase ×
      negotiated rate (whole blocks, `MAX_TRACE` 65 536 clamp), zoom-not-reset resize,
      rising-edge trigger with free-run fallback, stride-decimated clamped polylines, X/Y
      pairing over the shorter axis, NaN sanitised, params clamped without trust; the WIRE is
      the binding (per-frame resolution, rebind clears); the ring is the payload (the drain's
      same pass feeds traces and cv levels); `Scope::process` stays a no-op. Painter: the
      `scope.trace` token map, the wires' glow vocabulary, well at Full+Simplified, TRACE at
      Full only; unbound = the flat rest line. The manifest param ORDER pinned by a drift-gate
      test. **777 tests** (+11 model, +3 session, +2 smokes = 32), every golden bit-identical
      (stress `7bb06379bd6845e5` debug AND release, the three exec renders, canvas-render.wav,
      determinism), selftest 9/9, 17/17, every runnable clippy cell, 5 python gates, stamp
      `src 94f/2122410B`, log_check BASELINE moved. Parked and declared: phosphor persistence +
      the motion-token glow law (one data-first refinement increment for wires AND traces),
      min/max envelope decimation, larger display tiles (a layout-token increment), the other
      eleven display modules, offline scope previews (the ring is a LIVE publication). Sealed
      **`sync-wo013-inc6.zip` (16 entries)** — WAITING, stacked on inc2 + inc7.
- [x] **WO-012 inc 2** (2026-09-29, seventh session) — **the live audio session**: the UX
      pivot's first build, checklist item 6 exactly as re-sequenced (plan of record
      `WO012-INC2-PLAN.md`, fifteen decisions before the code + the postscript correction —
      `stage` supersedes and counts, it never refuses, so the retry flag does not exist and the
      visible refusal is the BUILD's in the executor's own words). `LiveSession`
      (`sparq-app/src/ui/live.rs`): two-phase open, the executor built for the NEGOTIATED config
      through the new `bridge::build_with_map_at` (`build_with_map` delegates at the render
      constants — bit-identical, pinned), the callback owning the `AudioEngine` (`render_block`
      and nothing else), STOP = join → measured evidence line → `reclaim()` drained → the
      engine drops on the control thread. The ONE op→sync door: `CanvasState` classifies every
      `Op` exhaustively into `PatchChanges` (structural / param-dirty / inaudible) at the single
      `commit` choke point undo/redo share — params cross as `set_params` with ZERO re-stages
      (no click mid-drag), structural edits and master handovers re-stage at the boundary
      (`inherit_runtime` keeps the transport clock). The bounded per-frame drain
      (`read_meters` + `read_analysis`, fixed scratch, last-wins) fills `CanvasState.levels` —
      the CONTINUOUS half of live wire levels; the painter path untouched. The ring extension
      riding it: `MeterUpdate.port` (one entry per audio OUTPUT port via the new
      `Executor::with_audio_out`, `FOLDED` sentinel for cv-only nodes, single-output entries
      bit-identical to the folded readings — pinned against an offline reference executor) and
      block-rate cv outputs publishing as one-sample analysis waveforms (a cv wire's LIVE level
      reads the inc-5 magnitude rule; the tap now publishes 3/block — its refusal-count test
      moved in words). Honesty lines in the house shape; `Removed`/`Failed` ends the session in
      ONE line, canvas untouched. Backend: manual null for the audit/tests (pumped by the
      smoke; `capture_frames` is the device-side proof), `wasapi-shared` for a Windows window,
      paced null elsewhere WITH a line saying so. Two documented driver doors joined the
      headless convention (`param_edit`, `connect_ports`). **766 tests** (+4 workspace, +5
      session, +5 smokes = 30), every golden bit-identical (three pinned exec renders,
      canvas-render.wav `d7ad294e…`, stress `7bb06379bd6845e5` debug AND release, determinism
      `0f5c3e86c7f117a9`, selftest 9/9, `modules --strict` 17/17, probe_alloc 0/5 000,
      throughput 262.8×), stamp `src 93f/2081612B`, log_check BASELINE moved with the seal.
      test006 grew steps J–K. Sealed **`sync-wo012-inc2.zip`** — WAITING for the device,
      stacked on the still-waiting inc7. Parked and declared: the scope screen (the named
      follow-on), per-port meter DISPLAYS beyond the wire glow, transport/arrangement binding
      (WO-009's), multi-reader epochs, `ui-window` release link in the 1 GB sandbox (the clippy
      cell ran; the link stays a SATURN job).
- [x] **WO-008 inc 7** (2026-09-29, sixth session, second increment) — host-side `required`-
      unconnected enforcement (the checklist's named next candidate; plan `WO008-INC7-PLAN.md`,
      eight decisions before the code): `ExecError::RequiredUnconnected` in the build gauntlet
      (after the edge rules — an illegal wire is a more specific defect than a missing one —
      before buffers; graph × manifest order), INPUTS only per the compiled `Port::required`
      doc, ≥1 carried edge satisfies, no bypass exemption (declared). The dependents moved:
      the determinism world wires its spare from the chain gain (master-cone retune sensitivity,
      pin 4 nodes / 3 edges), the canvas demo's fourth node became a bare `out/main`
      (canvas-render.wav byte-identical: 1 920 046 B, sha256 `d7ad294e…` recorded), the
      unconnected-signal tests migrated to optional ports (their declared home) + the new
      refusal gate with its compliant twin, cross_thread's status pin tightened to `Ok` only.
      **Stress baseline moved ON PURPOSE and recorded: `7bb06379bd6845e5` · 2 383 swaps ·
      7 617 refused · 0 allocs, debug == release** (was `b42068ec7b206789` · 7 203 · 2 797);
      cross-thread ledger 21 509 blocks · 2 299 = 2 299 = 2 299 · 7 701 refused · 0/0. Docs
      moved same-session (module-api-v1 open list, field-table + schema rows, LATER.md struck +
      the canvas-badge park). 762 tests, every audio golden bit-identical, selftest 9/9,
      ui-audit 25 smokes, all runnable clippy cells. **#93** (the `out`-dir snapshot trap)
      logged + remedied. Sealed `sync wo008-inc7` — WAITING, stacked on inc15.
- [x] **WO-006 inc 1.5** (2026-09-29, sixth session) — after test004 attempt 3 granted the
      default period and stalled anyway: the exclusive rate envelope decoupled from the shared
      sieve (#91 — the sweep asks the driver at every standard rate; `exclusive_rate_for` keeps
      play/soak adjustments inside the envelope the open negotiates in), #76 SHIPPED (drift =
      delivered frames vs wall; the `IAudioClock` binding deleted with its epitaph; both §4b
      fixtures pinned at ≈0 ppm; attempt 3's shape pinned as the −49 % verdict; the falsified
      "implausible clock" SUSPECT text replaced; 2 s trust window; play/soak verdicts act on
      suspect drift), #92 logged OPEN with the wakes×buffer arithmetic and push mode named as
      the next lever, and `test004.bat` re-contracted for attempt 4 at 48 kHz (the [07] rate
      prompt; the acceptance amendment PROPOSED in the run sheet §2). 761 tests, goldens
      bit-identical (release re-verified incl. the three pinned exec renders), selftest 9/9,
      ui-audit 25 smokes, null conformance 9/9, paced-null drift +0.23/−0.62 ppm live, every
      runnable clippy cell clean. Sealed `sync wo006-inc15` — WAITING.
- [x] **WO-006 inc 1.4** (2026-09-28, fifth session) — the defect-#89 fix after test004
      attempt 2 opened exclusive at 3 ms and stalled (~33 ms every ~62.5 ms, half throughput, no
      clean tone — while the shared 10 ms engine soaked 2 h clean in the same session):
      default-first period ladder for coarse engines, round-UP alignment two-step, silent-shrink
      guard + honest open line — all pure data in `hal/period.rs`, Linux-pinned on the attempt-2
      device shape. Defect #90: test004.bat's summary said "still refused" when exclusive had
      OPENED and run rough; refused vs opened-but-not-clean now distinguished, banner current.
      **`tools/log_digest.py`** + `scripts\digest.bat` + end-of-run hooks in test004/test006/
      gates — the compact `-digest.log` copies the operator asked for (attempt 2's log: −54 %,
      verdict lines byte-identical, `--self-test` 16/16). 757 tests, goldens untouched by
      construction, both MSVC hal-wasapi cells clean. Sealed `sync wo006-inc14` — WAITING.
- [x] **WO-008 inc 6** (2026-09-27, fourth session) — `cv_interp = "spline"` PERFORMED: the
      parabola through the last three block values (causal cubic-Hermite equivalence recorded),
      exact for quadratic sweeps, bit-identical to `linear` on collinear knots, `linear`'s
      arrival timing, f64 + one rounding per frame, clamped to the wire's declared range
      (`CvRange::clamp_f64`; hold/linear deliberately unclamped — source bugs stay visible);
      the build refusal retired, the matrix cell / §17 / schema row / author guide re-worded and
      drift-pinned; `CvPlan` grew the two-slot history + range; `tests/cv_spline.rs` 8 gates +
      2 `port.rs` unit pins + contract_v1's refusal gate replaced 1:1; defect #88 (the guide's
      stale mixer clause) fixed in passing. 752 tests, every golden unchanged, stress hash
      unmoved. Sealed `sync wo008-inc6` — WAITING, stacked on wo014-inc6.
- [x] **WO-014 inc 6** (2026-09-27, third session) — `util/mixer` 0.2.0, the cv merge side:
      4→1 block-rate unipolar merge with declared mean-reduce, per-input gains appended last,
      identity default, clamped publish; the executor's fan-in refusal re-worded to its built
      remedy (the rule stands); `tests/mixer_cv.rs` 9 gates; the inc-5 scroll smoke hardened
      against param-count drift. 742 tests, every golden unchanged. Sealed `sync wo014-inc6`.
- [x] **WO-013 inc 5** (2026-09-27, second session) — the "Else column" pass: the rename text
      entry (`canvas::entry`, the RENAME row, the `Op::Rename` commit with its empty-clears and
      unchanged-no-op edges, the unified modal key feed), inspector scrolling (`compute_at` + the
      fixed-header visibility rule + the thumb; `Pan.center` routing; pinch-over-panel declined),
      per-cv wire levels (`NodeLevels` per-port entries, `wire_level` port-first, the bridge
      reading the executor's real cv values — executor untouched, stress hash unmoved), and the
      LOD pass (Dot hairline wires, text-free Simplified, the hatch/dash/double/chip state
      patterns at every level, shape-encoded dots). 733 tests, every golden unchanged, audit PASS
      25 smokes, test006 steps F–I. Sealed `sync wo013-inc5` — APPLIED 2026-09-27.
- [x] **WO-014 inc 5** (2026-09-27, this session) — the module set COMPLETE at seventeen:
      `ana/tap` (wave golden cross-validates == the sine golden), `dsp/scope` (the no-op display;
      scope-rig golden == the out/main golden, proving zero-cost bit-transparency), `out/main`
      (unity-bit-exact master with the metering hook); the `AnalysisUpdate` ring extending d8's
      publication to waveforms; and the LFO beat-rate decision DECIDED *and* SHIPPED — `mod/lfo`
      0.2.0 + `util/delay` 0.2.0 derive bpm module-side from the block-start tick (`TempoFollower`),
      reaching the delay's `set_tempo_sync`, additive so every prior golden is unchanged.
      713 tests, every golden unchanged, `modules --strict` 17/17, docs 17.
- [x] **WO-013 inc 4** (2026-09-27, this session) — live wire levels (the signature image):
      the toolkit-independent `canvas::levels` model, `bridge::node_levels` reading REAL executor
      meters, the painter's token-colour→glow blend, `CanvasState.levels` refreshed on render,
      audit smoke 20, and the `out/main` master-handover rule (3 tests). Not faked; the cv-wire
      level limit + rename/inspector-scroll/LOD declared for the next pass. Sealed together as
      `sync wo014-inc5+wo013-inc4`.
- [x] **WO-008 inc 5** (2026-09-27, this session) — the cross-thread hot-swap primitive
      (`sparq-kernel::sync::hotswap`, allowlist entry 6): d3's literal pointer swap, epoch-tagged
      retirement (grace period mechanical, drops control-side), deferral-not-forcing; `Module:
      Send` (pinned); `SharedEngine`/`AudioEngine` + d8's meter ring + the `EngineCmd` ring; the
      two-thread 10 000-mutation acceptance (0 failed blocks, 0 audio-thread allocs, ledger
      exact); selftest gate 9; CI miri cell widened to `sync::`; defect #85. 682 tests, every
      golden unchanged (stress hash in debug AND release). Sealed `sync wo008-inc5`.
- [x] **WO-014 inc 4** (2026-09-26) — `mod/lfo` + `mod/clk-div` (14 modules),
      lfo→svf bit-identical sweep, 16ths→÷4→membrane chain golden `914d9063ce9d8a0f`, seeded
      probability reproducibility, binary-exact-rate arithmetic gates. 665 tests, all goldens
      unchanged. Sealed `sync wo014-inc4`.
- [x] **WO-009 inc 1** (2026-09-26) — piecewise kernel Clock (ramps + exact
      bisection), `sparq-music` (broker + transport + zero-alloc queue), the executor's musical
      door, transport-driven drum-demo == the batch-3 golden, ±0-sample acceptance vs an
      independent integral, ADR-006 addendum. 655 tests. Sealed `sync wo009-inc1`.
- [x] **WO-014 inc 3** (2026-09-26) — membrane + env/ad + mixer (12 modules),
      drum-demo golden with kicks on exact frames, defect #84 (TOPS table-first), 627 tests,
      goldens unchanged. Sealed `sync wo014-inc3`.
- [x] **WO-008 inc 4 — CONTRACT V1** (2026-09-26) — multi-port `AudioCtx`,
      cv/event payloads travel, rms→filter acceptance bit-exact, drift gate + defects #80–#83,
      toml nested-AoT fix, log_check baseline moved. 614 tests, goldens unchanged, stress hash
      unchanged. Sealed `sync wo008-inc4`.
- [x] **WO-014 inc 2** (2026-09-26) — the audio-domain six (9 modules total), aliasing
      acceptance −166.8 dB measured, six+one goldens, zero-alloc gates, docs generator + new
      CI/gates stage, `set_phase` on the osc. Sealed `sync wo014-inc2`.
- [x] **WO-008 inc 3** (2026-09-25) — tasks 4–7: Engine + 10k mutation stress + latency map &
      compensated fan-in alignment + watchdog auto-bypass + determinism harness (caught a
      HashMap-order nondeterminism pre-ship). Sealed `sync wo008-inc3`; device work: the loaded
      soak (needs SATURN + HAL).
- [x] **WO-006 inc 1.3** (2026-09-25) — the defect-#79 exclusive device-period ladder
      (`hal/period.rs` pure + Linux-tested; `open_exclusive` rewired; test004.bat text updated;
      sealed `sync wo006-inc13`, run sheet). Device-verified: NOT yet — that is test004's re-run.
- [x] **WO-013 inc 3** (2026-09-25) — module browser + fuzzy search, inspector with touch
      sliders + per-node param state (`Op::SetParam`, one drag = one undo step), wire-end
      re-patch, shell routing + provisional keyboard feed, painter for all three, 3 new audit
      smokes, test006.bat + run sheet. Live wire levels deliberately NOT done (need executor
      taps — now they exist, the painter half is WO-013 inc 4).
- [x] WO-013 inc 2b — delivery-path fix (#78); **test005 PASS on SATURN 2026-09-25 13:37**
- [x] WO-013 inc 2 — the bridge; WO-014 inc 1 — module batch + `sparq exec`; WO-008 inc 1–2 —
      graph core + executor; WO-006 inc 1.2 — integer exclusive rungs (#77 fix)
- [x] P1/P1B/P1C device runs — first MSVC builds green (golden bit-identical, selftest 8/8, 1059× realtime)
- [x] WO-007 complete — module contract as data + code; WO-012 inc 1 — gesture core + egui shell;
      WO-013 inc 1 — canvas model + painter; WO-000…WO-005 — workspace, tokens, ADRs, mockups,
      Phase A/B/C1 (DSP toolkit, music systems)

## Environment notes (sandbox) — re-read every session

- **Message boundaries drop excluded path components — including PRODUCT files (defect #93,
  observed 2026-09-29):** the snapshot excludes any directory named `out`, `target`, `build`,
  `dist`, `node_modules`, `.git` and friends — and this repo legitimately ships
  `modules/out/main/sparqmod.toml` (stamp-COVERED). It vanished mid-session, upstream GitHub
  404'd on it (repo rebuilt/private), and only the operator's copy recovered it (byte-exact,
  hash-verified: `725bf07a…`, 2 691 B). **Session-start discipline: run
  `python3 tools/sync_check.py --quiet` FIRST; if `modules/out/main/sparqmod.toml` reports
  MISSING, restore it from `../sparq-recovery/modules-out-main-sparqmod.toml` (workspace root,
  outside the repo — durable) and verify the sha256 matches the stamp line before building.**
  The `/opt` toolchain and `.git` vanish at the same boundaries (both documented below).
- **`.git` is NOT durable in this workspace** (observed three times: 2026-09-25 twice,
  2026-09-26 the clone arrived without history and was re-inited; the 2026-09-27 clone DID
  arrive with the rebuilt history intact and commits held all session — treat that as luck, not
  as a change of rule). Do not rely on git for state;
  the integrity tool is `sync_check.py` (content hashes) and the seal is the zip. Re-init for
  in-session diffs if useful, expect it gone next session. **And the pushed tree can drift from
  the sealed one** (defect #86, found 2026-09-27): run `sync_check.py --quiet` against the
  PRISTINE clone at session start — if it fails, the drift list IS the repair list, and the
  bundle must carry the repairs plus the device-side forensics step.
- Toolchain wipes **between and mid-sessions** (again 2026-09-26, 2026-09-27 and 2026-09-29 —
  this session's clone arrived with `/opt` empty; the recipe below restored rustup/cargo/gcc in
  ~150 s and everything ran, cargo 1.98.1; within one conversation the install persists across
  turns). Lives in
  `/opt`, deliberately outside the snapshot. Restore:
  ```bash
  apt-get update && apt-get install -y curl ca-certificates gcc libasound2-dev pkg-config
  export RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo   # set BEFORE rustup: $HOME is /tmp here
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
       --default-toolchain stable --component rustfmt,clippy
  export PATH=/opt/cargo/bin:$PATH
  rustup target add x86_64-pc-windows-msvc   # cross-LINT only; never links
  ```
  (Setting RUSTUP_HOME/CARGO_HOME before the installer avoids the 2026-09-25 `/tmp/.cargo`
  gotcha entirely — the installer honours them.)
- Every cargo call needs the exports above (shell state does not persist between tool calls).
- **1 GB RAM is the binding constraint.** `ui-window`/wgpu tree: build/clippy with `-j 1` and
  `CARGO_PROFILE_DEV_DEBUG=0` (proven again 2026-09-27, 1 m 33 s clean). `ui --audit` +
  goldens: run release with `CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`
  **and `CARGO_PROFILE_RELEASE_DEBUG=0`** (observed this session: the repo profile's release
  debuginfo=1 OOM-kills `read-fonts` at the default -j; with all three the release ui build is
  minutes, not a corpse) — SATURN builds with the repo profile. TWO cells do not fit even
  single-job, both the `windows`-crate rustc OOM class: MSVC×ui-window (standing) and
  MSVC×bootstrap-audio (observed this session — cpal pulls `windows` for the MSVC target; the
  documented bootstrap-audio cell is the NATIVE one, which passes).
- Background processes do NOT survive between tool calls — run long builds synchronously.
- Default workspace build has zero third-party deps → `cargo test --workspace` is cheap (~70 s).
- Gates sequence (`just gates` equivalent, plain commands): `cargo fmt --all --check` → clippy
  cells → `cargo test --workspace` → 5 python gates (`tools/check_text_io.py`, `token_gen.py
  --check`, `token_audit.py`, `unsafe_audit.py`, `module_docs.py --check`) → release golden tests
  → `selftest --golden` → `ui --audit`. `sync_check.py` runs at seal time, not in gates.
- **After any product-code change, re-seal:** `python3 tools/sync_check.py --write --sync <name>
  --sent <date>` then `--quiet` (verify) and `--self-test` (prove failable), rebuild the zip, and
  keep heading = list = contents counts equal in `SYNC.md` (the off-by-one class is checked, not
  hoped). New `scripts/*.bat` files ARE stamp-covered (covered set 134 files); `fp` counts only
  the five src roots (86f). **AND move `tools/log_check.py`'s BASELINE with the seal — defect #83
  was two increments of forgetting exactly that.**
- Python text I/O must be explicit `encoding="utf-8"` (+`newline="\n"` on write) —
  `check_text_io.py` gates it. `.bat` files: pure ASCII, CRLF, no unescaped parens in echo text
  inside blocks (#42/#69).
- Avoid literal workspace paths inside Python source (sandbox rewrites them); use relative + cwd.

## Open defects / debt (tracked in PHASE0-WORKORDERS.md build-log tables)

- **#79 — FIXED (inc 1.3), DEVICE-VERIFIED (attempts 2–3)** — the asking works: exclusive opened
  on attempt 2, and attempt 3's open line landed exactly on the predicted default-first shape
  (`960 fr (10.00 ms)`, no shrink note). What attempt 3 also proved: the unsustainability beside
  it is NOT period-specific — reclassified as #92. Acceptance remains the test004 re-run.
- **#80 — FIXED (contract v1):** the compiled HOA adapter offer named `spa/objects`; split into
  `HoaEncode`/`HoaDecode` with the matrix's ids, direction-aware, and objects↔non-spatial now
  refuses (no named converter). Caught by the new drift gate on its first run.
- **#81 — FIXED (contract v1):** the matrix's `cv→audio` adapter row was stale
  (`syn/sine-or-offset`) against the ADR-005 addendum's settled `util/offset`; the table now
  carries a `decided =` line naming the gate run.
- **#82 — PINNED + ESCALATED (contract v1):** `mono → ambisonics:N` is "compatible fan-out" in
  BOTH copies (the table's case order and the code's match arms agree) — replicating mono into an
  AmbiX bed is not valid spatial audio. Pinned by
  `the_mono_fanout_case_precedes_the_spatial_case_in_both_copies_flagged_for_review`; the fix is
  a matrix-review decision (`reviewed = false` is pending), not an implementer's side quest.
- **#83 — FIXED (contract v1):** `log_check.py`'s BASELINE frozen at WO-007 (388/68f) through
  two seals; moved with the seal (627 at wo014-inc3) with a provenance comment, and the seal
  recipe above now names it as a step.
- **#84 — FIXED (WO-014 inc 3):** `classification.top`'s closed domain lacked `env`/`mod` in
  both copies while Appendix B's ids and WO-014's artefact paths name them — `env/ad` could not
  declare its own top. Fixed table-first (TOPS 16 → 18) with the field table's first drift pin
  (`the_tops_vocabulary_matches_the_field_table`).
- **#85 — FIXED (WO-008 inc 5):** the exec evidence hashes were recorded without their
  durations — the CLI default renders 5 s (`drum-demo` → `de216da86b2a1869`) while the goldens
  are the 2.8 s/1 s/2 s renders; all three reproduce EXACTLY at their pinned durations (verified
  debug == release). The standing order now carries explicit `--seconds` everywhere the hashes
  appear, so a device operator compares like with like.
- **#86 — FIXED-IN-BUNDLE (WO-008 inc 5): the GitHub tree drifted from the sealed tree.** The
  pristine clone fails `sync_check --quiet` against its own stamp: module-api's lib.rs is 230B
  smaller than sealed and sync_check.py lost sparq-music from its covered/fingerprint roots (86f
  vs the stamped 89f). The history rebuild dropped bytes; the zips are truth. Repaired:
  sync_check.py restored to the six-root definition, and the inc-5 bundle carries BOTH files as
  declared drift repairs — **the device must copy its sealed versions to `logs\` BEFORE
  extracting** (the run sheet says so) and send them back for the diff of what the rebuild lost.
- **#87 — FIXED (WO-014 inc 6): the inc-5 inspector-scroll smoke was overfitted to one module's
  panel.** Its fixed two-finger drag distance (480 px) had been tuned so the mixer's LAST row was
  revealed at the 20-param `max_scroll`; when mixer 0.2.0 grew to 24 params the drag fell short
  and the gate failed on a healthy feature. No device exposure: the fragile cell only fails in
  combination with mixer 0.2.0, and both halves ship in the same bundle (`sync-wo014-inc6.zip`) —
  a device that applied wo013-inc5 alone runs the old smoke against the old mixer and passes.
  Fixed the right way: the smoke now asserts the CONTRACT (the panel overflowed before the drag;
  after it, some previously-hidden row is visible AND touchable where drawn) instead of a chosen
  row index or a distance. Lesson for smoke authors: a gate that encodes one module's arithmetic
  instead of the behaviour's shape will fail on the next additive version bump — which is exactly
  when it should pass.
- **#88 — FIXED (WO-008 inc 6, docs-only, found in passing): the author guide's §11 still called
  `util/mixer` "Phase 1, not yet built"** — stale since WO-014 inc 6 shipped the cv merge side.
  The same bullet carried the spline-refusal sentence this increment had to rewrite anyway, so
  both halves now tell the truth; no code, no device exposure. The lesson, turned on docs: a
  bundle that changes a contract sentence should grep the author guide for that sentence in the
  SAME session — the guide is the copy module authors actually read.
- **#89 — FIXED (WO-006 inc 1.4), open-path DEVICE-VERIFIED (attempt 3, 2026-09-28 evening):
  the open line landed on the predicted `960 fr (10.00 ms)` default-first shape, granted, no
  shrink note. The runtime reading — "the 3 ms period is what the driver cannot sustain" — was
  FALSIFIED by the same attempt: the stall returned, wall-identical, at the 10 ms default. The
  open-path hygiene stands (default-first is still the right ask); the runtime phenomenon is
  #92.** Original finding, kept for the record:
  The exclusive stream opened at 288 fr (3.00 ms) — the driver's reported minimum / alignment
  granularity — and stalled: one ~33 ms stall every ~62.5 ms (159 late wakes / 10 s), half
  throughput (49.1k fr/s vs 96 kHz), drift −491 656 ppm, no clean tone; 0 budget overruns and 0
  FIFO starvations (the pump kept every promise it could see), and the shared 10 ms engine on the
  same endpoint soaked 2 h / 719 895 wakes / 0 xruns in the same session. **The third lying-`min`
  face: `Initialize` ACCEPTS the period and the engine cannot SUSTAIN it** — invisible to every
  open-path probe. The attempt-1 log copy in the repo is truncated, so which door produced the
  3 ms ask (a `GetDevicePeriod` min now reading 3 ms, or the two-step adopting a 288-fr
  granularity against a 10 ms ask) is not pinnable from the evidence — the fix closes all three
  doors: default-first ladder when min > block, round-UP two-step, silent-shrink guard (< ¾) with
  the ask printed in the open line. Pure data in `hal/period.rs`, Linux-pinned on the attempt-2
  device shape; arithmetic + fix story in `docs/hal/windows-notes.md` §4d. Parked in LATER.md
  (updated by inc 1.5): runtime stall DETECTION's instrument half shipped — delivered-vs-wall
  drift with a verdict (§4e) — and the reopen half stays parked with attempt 3's evidence
  attached: reopening within the period ladder buys nothing while the stall is
  period-independent.
- **#90 — FIXED (WO-006 inc 1.4): `test004.bat` misdiagnosed attempt 2 inside the log it asked
  the operator to send back.** The summary's `EXCL_OK` keyed off rc alone, so an exclusive stream
  that OPENED and ran rough printed "exclusive still refused — the [04] probe table is the
  diagnosis" (a table that, correctly, was not in the log), and the banner still announced
  "increment 1.2" two increments later. Fixed: `EXCL_OPENED` captured via `findstr` on the [04]
  output, refused vs opened-but-not-clean summary lines, hints naming #89's runtime signature,
  expectation text naming both legal periods (960 fr / 1152 fr) and the 288-fr signature. The
  lesson: an acceptance script's verdict text is part of the acceptance — a wrong diagnosis in
  the send-back log costs a round-trip just like a wrong fix would.
- **#91 — FIXED (WO-006 inc 1.5): the exclusive rate envelope was probed through the shared
  sieve, and the CLIs adjusted inside the wrong envelope.** `capabilities()` asked the exclusive
  rungs only at rates the SHARED f32 probes had accepted — on SATURN's UMC 204HD the engine
  refuses every shared rate but its own 96 kHz mix, so caps printed `exclusive 96000` as
  measured truth while the "192k" driver was never asked about 44.1–192 kHz exclusive; `play`
  and `soak` then consulted `supports_rate` (the shared list) even for exclusive streams and
  bent the standing 48 kHz ask UP to 96 kHz — the one rate the exclusive path had proven
  pathological at (attempt 3's own `adjusted` line is the evidence). Fixed: the exclusive sweep
  asks the DRIVER at every `PROBE_RATES` rate plus the mix rate (four rungs each; the field doc
  carries the rule — each envelope has its own arbiter, `Initialize` remains the final one), and
  `Capabilities::exclusive_rate_for` (pure, Linux-pinned) keeps exclusive adjustments inside the
  exclusive envelope (nearest verified rate, ties to the lower; empty envelope → `default_rate`
  so the open ladder still refuses honestly with its probe table). Found by reading attempt 3's
  log against the probe code — recorded in `docs/hal/windows-notes.md` §4e.
- **#92 — OPEN (device-side; logged by attempt 3, 2026-09-28 evening): the exclusive event-stall
  phenomenon is period-independent.** ~16 stalls/s, ~33–40 ms wall each, identical at a 3 ms and
  a 10 ms period at 96 kHz; 158 late wakes / 10 s, half throughput (7756 blocks ≈ 49.4k fr/s),
  no clean tone — with 0 budget overruns and 0 FIFO starvations (the pump kept every promise it
  could see, twice) and the same session's shared runs all green (the engine's 2112-fr buffer
  over ~960-fr ticks has 2.2× headroom; event-exclusive legally has NONE — `hnsPeriodicity` ≡
  `hnsBufferDuration` under `EVENTCALLBACK`, MSDN's rule). The damage arithmetic is exact on
  both attempts: **delivered = wakes × buffer** (517 × 960; 1718 × 288) — every swallowed
  cadence is audio the device never receives. Root cause is NOT resolvable from the sandbox
  (candidates: the USB class driver's exclusive URB/event scheduling; the RDP session — no
  exclusive run has ever happened on this machine without RDP; driver power states). Inc 1.5
  removes the forced 96 kHz (#91's fix — attempt 4 discriminates rate-coupling) and ships the
  instrument (#76) that makes the digest self-diagnosing. If attempt 4 shows the triple at
  48 kHz: rate-independent, and the named lever is **push-mode exclusive** (LATER.md — periodicity
  0, no `EVENTCALLBACK`, buffer 2–4 periods, self-paced pump; the shape the audio engine itself
  runs on this endpoint), its own increment with its design burden (pump pacing discipline,
  ADR-004's "event-driven" wording, late-wake semantics under a policy cadence). Evidence:
  `test004-digest.txt` [04] of both 2026-09-28 runs; arithmetic in `docs/hal/windows-notes.md`
  §4d/§4e.
- **#93 — REMEDIED (WO-008 inc 7, environment/delivery class): the sandbox workspace silently
  drops any directory named `out` at message boundaries — and `modules/out/main/sparqmod.toml`
  is stamp-COVERED.** Mid-increment, `sync_check --quiet` reported the file MISSING (the
  snapshot exclusion list treats `out/` as a build artefact directory; `.git`, `target/` and the
  `/opt` toolchain drop at the same boundaries — documented, expected — but this is a PRODUCT
  FILE). Upstream recovery failed: GitHub 404s on the path (the repo was rebuilt — head `daeda8c`
  is not this clone's lineage — or went private mid-session; the same class as defect #86,
  "the pushed tree can drift from the sealed one", quite possibly the same trap on the packaging
  side — worth checking whether the pushed tree still carries `modules/out/`). The operator
  supplied the file; it was restored BYTE-EXACT and hash-verified against the sealed stamp
  (`725bf07a1fb532d32ebf04043715e680ef1e67eafa22f85dfebc75a6b1f54dd0`, 2 691 B) before use.
  Remedies in place: a durable recovery copy at the workspace root's
  `sparq-recovery/modules-out-main-sparqmod.toml` (outside the repo, no excluded path
  component), the session-start discipline in the environment notes below, and this row. The
  device tree is unaffected — bundles never removed it, and SATURN's own `synccheck.bat` would
  name it if it ever went missing there.
- **#96 — REMEDIED (round-4 S9 wrap-up, 2026-10-02; environment/delivery class — the #93 trap's
  third strike): the between-turn reset dropped `modules/out/` from the SEALED tree, and no
  byte-exact copy survived anywhere.** `sync_check` named the one file of 162 MISSING at the
  wrap-up turn (the gate earning its keep a third time). Unlike #93 there was no operator copy
  to restore from: the repo answers 404 (private — #94's other half), `.git` and `target/` do
  not outlive turn boundaries, the three checkpoint zips are changed-file overlays that never
  carried the file, and the sandbox's own undo snapshot applies the same `out/` exclusion (it
  was checked). Remedied per #94's pattern: REBUILT from its generated doc
  (`docs/modules/out-main.md`) and the `OutMain` implementation, PROVEN semantically identical
  at both layers — `module_docs.py` regeneration byte-matches the checked-in doc (24/24) and
  the real Rust decoder (`sparq_module_api::decode::decode`, the call `Registry::register` and
  `modules --strict` ride) validates all 24 manifests with out/main's parsed vocabulary
  eyeballed field-for-field against the doc. The file's header declares the reconstruction.
  Re-stamped `sparq-round4-2026-10-02`: the row moved (`86608e10…`, 4 087 B — comments are the
  only difference from the sealed `3b0a619f…`, 2 544 B); the fp did NOT (`src 96f/2694477B`
  walks `.rs` files only — which is why every doc quoting the fp stays true). Remedies in
  place: the durable recovery copy at `../sparq-recovery/modules-out-main-sparqmod.toml`
  RE-ESTABLISHED with the reconstructed bytes (#93's copy died with the lost sandbox — the
  session-start discipline in the environment notes below is what failed to happen this
  session; it stands), the pack carries the file, and the device's `gates.bat` re-proves it
  end to end after apply.
- **#97 — REMEDIED (device report 2026-10-07, "sparq becomes unstable when the zoom buttons are
  used while the observatory is loaded"; host-painter class): `dash_segments`'s f32 walk stalled
  at EVERY toolbar zoom except exactly 1.0.** The egui display-list painter decomposes
  dashed/dotted strokes into segments (the SVG back end rides `stroke-dasharray`, which is why no
  headless gate ever saw it). Per-step f32 drift lands `phase` within an ulp of a boundary —
  `on*z` and `(on+off)*z` are different roundings of the same products — where `at + (on-phase)`
  rounds back to `at` and the walk stops advancing: the dash branch pushed zero-length segments
  until the OOM killer took the process (reproduced: SIGKILL 3.5 s into the regression test on
  the old walk), the gap branch spun with the UI thread frozen. An exact-f32 model over the real
  at-rest geometry (130 dotted polylines out of `preview.svg`) hangs **130/130 at every painted
  ÷1.2/×1.2 ladder value** (0.8333…, 0.6944…, 0.5787…, 0.4823…, 0.4019…, 0.36, 0.432, 0.5184,
  0.6221, 0.7465, 0.8958); only z = 1.0 survives (1/4/5 are exact binary there) — the default
  view is clean and the FIRST zoom click kills the shell. Fixed in
  `crates/sparq-app/src/ui/displaylist_egui.rs`: the walk computes in f64 plus a totality guard
  (a step that fails to advance ends the run; what it drops is sub-ulp ink). Two regression tests
  pin the exact ladder values, proven failable per the house rule. No golden moves: the dash walk
  is host-painter geometry, not IR. Cell measured: `sparq-app --features ui` 45/46 (the one red
  is the environment-sensitive `ui::live` level-follows test, which fails identically on the
  pristine tree in this sandbox). **DEVICE-CONFIRMED (session 8, r7, the operator's own words):
  "the zoom is working without OOM" — the whole ladder, with the Observatory loaded.**
- **#98 — REMEDIED (device gate log 2026-10-07, stage 4's hash `5acfc1e6…` ≠ the harness pin
  `bdf59cdb…`; artefact-staleness class — the #78/#93 lineage): the checked-in
  `instruments/observatory/observatory.wasm` (`3f775856…`, 206 081 B) PREDATED INC4's
  `cell_ground` draw-order fix.** The core moved (ground → body → chrome, "caught by LOOKING at
  the render"), the harness golden re-pinned to `bdf59cdb…`, and the component was never rebuilt
  — the device faithfully rendered the OLD logic. Proof: the rebuild from current source
  (documented recipe, rustc 1.99.0 + wasm-tools 1.261.0) exports `observatory_core::render::cell_ground`
  and the checked-in one does not; the checked-in `preview.svg` (the device's stage-6 sync-back)
  carries all sixteen cell grounds at 78–100 % of file — ground painted OVER the bodies, the
  exact bug the fix moved under — while the current core's harness output puts the first ground
  at 0.1 %; and the host-side pipeline is parity-proven by reading (both IR writers
  field-for-field identical, no `preserve_order` on either side, fixtures all-LIVE at replay-now
  so `snapshot(now)` == `iter()`, frame/params/LOD identical). Remedy: the package reassembled
  around the rebuilt component **`4b29a5ae…`, 206 316 B**, the stale `preview.svg` dropped (the
  device's stage 6 regenerates file 5 of 5 from the fresh component); the cross-boundary pin does
  NOT move — bdf59cdb is the current core, harness goldens green in the sandbox (6/6). Standing
  lesson parked in LATER.md: a golden re-pin must force a component rebuild — the pin moved when
  the renderer moved and the package did not. **DEVICE-CONFIRMED (session 8, r7): [D] stage 4
  reproduced `bdf59cdb…` EXACTLY over the WASM boundary on component `4b29a5ae…` — GATE: PASS,
  the whole chain; stage 6's regenerated `preview.svg` came out the same 1 554 915 B as the stale
  one (the ground fix reordered items, it added and removed none).**
- **#99 — REMEDIED (the same gate log, stage 4's fuel `12 707 802 vs 12 401 514`; gate-
  measurement class): the golden stage compared the FIRST draw against the second — a cold heap
  against a warm one, not two identical runs.** wasmtime 49 meters `memory.grow` per page
  (`translate_memory_grow` charges `operator_cost.variable().memory_grow_per_page`, read in the
  pinned source), and the guest's first draw grows linear memory for the wall's items/strings;
  later draws reuse the warm free-list. Both costs are fully deterministic — TWO independent
  loads each burned EXACTLY 12 707 802 on their first draw (smoke's and the golden's) — so the
  stage's failure words ("find the unseeded randomness or the clock read") named the wrong class;
  there was no randomness anywhere. Fixed in `runtime/stages.rs::golden_stage`: one discarded
  warm-up draw before the measured pair; stage 5 KEEPS the cold draw on purpose (the first
  production frame pays it — it is the worst case the declared budget must cover). The failure
  words grew the fuel-only sentence. **Companion ruling (not a defect): `capabilities.max_fuel`
  re-baselined 2 000 000 → 32 000 000, operator ruling 2026-10-07** — stage 5's measurement
  (draw 12 707 802 cold / 12 401 514 warm; process blocks 9 260; memory 1.2/3.2 MB of 64 MB) is
  exactly the re-baselining fact the r4 session built stage 5 to produce; plan §5.8's 2 000 000
  was its own words "fuel is essentially all `draw`", declared before any runtime could meter it.
  Recorded in `gen_manifest.py`'s comment, the package README and WO020-STATE session 7.
  **DEVICE-CONFIRMED (session 8, r7): stage 4's two runs came back EQUAL (warm, 12 390 844 fuel
  each — the warm-up fixed the comparison), and stage 5 passed inside the re-baselined class
  (cold draw 12 680 530, worst block 9 260 of 32 000 000; memory 1 245 184 B / 3 211 264 B of
  64 MB).**
- **#100 — REMEDIED (INC6 S2 sandbox session, 2026-10-08; found by building the typed panel
  band): a manifest-parsed `enum` param arrived with a ZEROED range (`min: 0, max: 0`), so
  every picker selection or gate edit on it clamped to 0 — "no change".** `param_descs`'
  non-numeric arm zeroed the range back when enums were refuse-only in v0 (the ParamDesc doc's
  own words); WO-020 INC4 §8.4 then made enums settable through the dropdown picker — and the
  picker's own interact test hand-built its enum with `max = options.len()−1` and a comment
  naming exactly this domain ("the slider maths clamps to this, so a picker selection outside
  it would read as 'no change'"), while the DISCOVERY door never caught up. Invisible until S2:
  the first surface to route manifest-parsed enums through the picker was the Observatory's
  band (CELL/STREAM/LAYOUT chips) — and `selected_cell` could never leave 0, so the wall could
  never swap cells. Fixed at the source (`param_descs`: an enum's domain IS its option-index
  range, `0..=options.len()−1`; Text/Blob stay zeroed — refuse-only, no editor); the ParamDesc
  doc corrected. Regression-pinned three ways: the model test on the SHIPPED manifest
  (`selected_cell` 0..=15, `layout` 0..=4, its default 4 in-domain), the band's picker-route
  interact test, and smoke 57's CELL-chip row. Injection-proved: restoring `max: 0.0` fails all
  three + the audit row. **No device impact yet — nothing on SATURN routes a manifest-parsed
  enum through an editor until INC6 lands there; the fix rides the S2 seal.**
- #69 — `build.bat` cmd-parser death: probe ships, culprit statement not yet named (stays open)
- compat-matrix mirror in `port.rs` vs `docs/api/compat-matrix.toml` — **re-worded (contract
  v1):** the drift gate pins the two together; full deletion waits on restructuring the table's
  prose `when` cells into machine predicates (review-packet change; ADR-005 addendum records it)
- WO-002: two typefaces still `chosen = ""`; WO-004: human protocols unrun; ADR-009 still `draft`
- `sparq-app/src/modules.rs`: no unit tests (CLI smoke only); `sparq-module-api`: no golden render
  of its own
- WO-013 inc 3 v0 limits (all parked in `LATER.md` with reasons): no numeric param entry, no
  inspector scrolling, no enum/text/blob editing (manifest v1 `options[]` first), single-selection
  inspector only, provisional keyboard feed for the browser, no browser scroll gesture
- WO-008 declared limits (parked in `LATER.md` §WO-008): ~~single-owner Engine until the kernel
  hot-swap primitive lands~~ and ~~meters not yet ring-published cross-thread~~ both SHIPPED
  (inc 5); remaining: compensation aligns PLAIN audio fan-in arms only; `required`-unconnected
  inputs not host-enforced (deliberate — the stress baseline moves with that change); `spline`
  interp refused until implemented; watchdog→UI red hairline is a WO-013-side painter increment;
  new from inc 5: one control + one audio thread (multi-reader epochs later), per-node meters
  fold a node's outputs (per-port later), `play` still pumps the static WO-005 graph (HAL
  integration waits on WO-006's acceptance)
- MSVC × `ui-window` clippy cell: never run anywhere (sandbox OOM, no CI cell)
- Device-side baselines that MOVE with the waiting increment (wo008-inc7; wo006-inc15's are on
  the device): gates.log test count **762** (log_check already moved — #83's discipline), stamp
  **`src 92f/2021628B`**, `sparq modules` count **17** (unchanged — no manifest moved),
  `selftest --golden` prints **PASS (9 gates)** (unchanged), `ui --audit` **PASS (25 smokes)**
  (unchanged count; the demo patch's fourth node is now a bare `out/main`), exec's three pinned
  evidence hashes are UNCHANGED (`demo` 2.8 s `53de3b1f3f40e3c9`, `mod-demo` 1 s
  `1621e1f65b1b64e1`, `drum-demo` 2 s `f2303f13aa0cf299` — enforcement only ADDS build
  refusals; no render path moved), no new audio goldens. **MOVED ON PURPOSE (inc 7's recorded
  intent, not a regression):** the stress evidence line — `7bb06379bd6845e5` · 10 001 blocks ·
  2 383 swaps · 7 617 refused (was `b42068ec7b206789` · 7 203 · 2 797) — and the cross-thread
  ledger's numbers. **UNCHANGED and verified:** `canvas-render.wav` — 1 920 046 bytes, sha256
  `d7ad294ea0b5e6e960f8dbc7f231dcbd84edec327e6352e80dc2f6ed786ec0b7` (the demo's master and
  samples did not move; test006's [05] A/B keeps its shape). Inc15's drift expectations stand
  (healthy runs read `accepted frames vs wall` within a few hundred ppm of 0 in BOTH share
  modes); the digest remains the send-back artefact (the full logs stay on the device).

## WO-020 INC5 — slices 1–2 (2026-10-07, sandbox session on `wo020-inc5`)

* **Slice 1 (MEASURED):** `streams-net` landed — `sparq-streams::{fetch,broker}`, the
  `HttpTransport` (ureq 3 + rustls), `BrokerProvider` behind the same `StreamProvider` trait, the
  real `sparq streams probe --live|fetch|tail` verbs, the registry's `accumulate` column (the
  three SWPC summary feeds; INC3 finding #7 discharged), the python recorder's key-redaction fix.
  Gates: root 1033/0/1, streams 96/0 both cells, host-wasm+streams 28/0, app 34/0, observatory
  124/0, every python gate, fmt/clippy clean, zero-dep promise by `cargo tree`. LIVE: smoke fetch
  green; probe 22/23 HTTP 200 + FIRMS KEY NEEDED in words. `record-fixtures` stays a refusal
  naming its owner (tools/streams_record.py) — a recorded ruling, reversible.
* **Slice 2 (WRITTEN — device/CI first compile, the §8 memory verdict re-confirmed):**
  `sparq-host-wasm::runtime` — the wasmtime 49.0.2 loader (identity cross-check, prepare-before-
  anything ×2 instances, fuel→Overrun, the memory limiter, wasmtime's own compile cache so no
  unsafe enters Tier-2 host code), the four doors, the gate's runtime stages 3/4/5m/6 (smoke
  mirror, golden ×2, measured budgets, visual + preview.svg + the at-rest publish),
  `validate::run_gated`, `Registry::register_instrument` (the SharedFactory door, tested here),
  the noop fixture REBUILT + sha-pinned (`df14d18f…`, 53 747 B), the §7 test plan as 12
  integration tests, `sparq mod validate` wired for the FULL chain under the app feature, and
  `scripts/test007.bat` (the §15.2 run-sheet A–L) + the gates/build/CI feature-on cells.
  Written against the REAL bindgen expansion (the pinned generator run over the frozen WIT —
  recipe in WO020-STATE.md) and the vendored 49.0.2 sources.
* **Outstanding:** the device run (test007 A–L) and **INC5b** (launch registration, PLAY #58,
  the shell's live-provider swap) — named in LATER.md and the STATE card.
