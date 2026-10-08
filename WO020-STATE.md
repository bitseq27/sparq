# WO020-STATE.md — state record (The Observatory, WO-020) — **INC1–INC4 CLOSED; INC5 slices 1–2 CLOSED; r7 device run: [D] GATE: PASS (session 8); gates.bat row + [G]/[I] eyes outstanding; INC6 slices S1–S5 CLOSED in the sandbox (session 9: the resizable card, the typed panel band, the store + the STREAMS tab, the live plane's seams + the device-first launch/driver, the card's RATE row + convergence); the DEVICE round outstanding (test007 [K] re-run + test008)**

**Last touched:** 2026-10-08, the INC6 session 9 (fresh sandbox on the re-published main
`b2445d6` — the operator's r8-seal re-publish; the recovery note is the INC6 section's first
paragraph) — **slice S1 landed sealed: the resizable instrument card** (ruling O-1, plan D15:
`Node.size` + `Op::ResizeNode` + the half-face default + the corner handle + the clamp + the grid
snap; workspace 1047/0, audit PASS 71 rows, proven failable by three injections), **slice S2
landed sealed: the typed panel band** (plan D16: the manifest's 27 `[[ui.panel.widgets]]` typed
into the card's band, enums through the EXISTING picker, toggles/sliders through the param door,
the at-rest sentence in words; workspace 1054/0, audit PASS 75 rows, two more injections; found
and fixed **defect #100** — manifest-parsed enums arrived with a zeroed range, so no picker
selection could ever change them), **slice S3 landed sealed: the overrides store + the
STREAMS dock tab** (rulings O-2/O-3/O-4, plan D18/D20: the user-data key/cadence store with the
10 s floor refusing in words and env winning over the file, the broker's due/stale-horizon
reading the overrides, the CLI's every door composed through the store, and the dock tab's
honest rows — fixture statuses under an `AT REST — fixtures` header, masked key fields, the
precedence visible; workspace 1055/0, streams 106/0, audit PASS 80 rows with `ui,streams`,
three more injections; the acceptance's live-key-flip half rides S4/the run sheet, as planned),
and **slice S4 landed sealed (its sandbox half): the live plane's seams** (plan D17/D19: the
D19 token `instrument_display_hz = 15` ruled into layout.toml, the `LiveDisplay` seam + the
pacer (edits force, five consecutive overruns bypass with §3's words), the live/at-rest band
switch, the driver's hermetic `poll_once` + the thin device thread, the provider swap (LIVE vs
AT REST headers), and the DEVICE-FIRST-COMPILE launch registration (`instrument_launch`:
load_package → the pending shelf + the registry's factory door — PLAY's #58 lifts
structurally; desk-checked call by call, `scripts/test008.bat` is its run sheet); workspace
1061/0, audit PASS 84 rows with `ui,streams`, three more injections; the S4 acceptance rows
[B]/[D]/[E] are DEVICE-PENDING by construction), and **slice S5 landed sealed: the card's RATE
row + convergence** (D20's second door — the host row on the info band's precedent, its field
writing the SAME per-stream override the tab does, its words naming the governed stream and the
shared-feed truth and following the CELL dropdown the same frame; the card chrome grew 168→208
so the default card is 1120×768 and the maximum 2208×1328 — the O-1 BAND numbers untouched;
the convergence sheet re-shot at Full LOD, sha `56e446d3…` recorded in the INC6 section, the
live-wall PNG re-shoot rides the device round; the mockup audit re-run at its unchanged
standing set; workspace **1062/0**, audit PASS 88 rows with `ui,streams`, three more
injections). **INC6's sandbox work is COMPLETE — what remains is the device round.**
See the INC6 section below. **Previous touch:** 2026-10-07, the INC5 session (fresh sandbox on the re-published main `a9dbf31`;
branch `wo020-inc5`) — **slice 1** landed the stream plane's live half (`streams-net`: fetch.rs,
the broker, `BrokerProvider`, the real CLI verbs — ALL MEASURED here, incl. a 22/23 live probe);
**slice 2** wrote the instrument runtime (`sparq-host-wasm::runtime`: the wasmtime loader, the four
doors, the gate's runtime stages, the noop fixture rebuilt + sha-pinned, `scripts/test007.bat`) —
device-first-compile by the §8 memory verdict, re-probed and RE-CONFIRMED this session (winch
alone still pulls cranelift). See the INC5 section below. **Previous touch:** 2026-10-06, delivery session (same sandbox, after the INC4 seal) — **INC1–INC4
PACKED FOR THE DEVICE**: `sparq-update-2026-10-06-wo020-inc4.zip` + `WO020-INC4-RUN-SHEET.md` +
the closing `handoff/sparq-wo020-delivery.bundle` (see **Delivery** below). **Previous touch:**
the INC4 session — **INC4 CLOSED**:
the host painters over INC3's IR, the dropdown picker (round-4 OWED item 11 discharged), camera
FOCUS, the D6 wall-card sizing, the browser's instrument grouping, the at-rest wall in the shell,
`sparq instrument render`. Plan of record: `WO020-OBSERVATORY-PLAN.md`. Remaining: **INC5 is
device-only by construction** (the run sheet's §4), INC6 is deferred/gated.

> **Environment note (survives a re-read):** the sandbox snapshot does NOT persist the installed
> toolchain or `.git`. This session re-installed rustup (via python — no curl/wget) + `gcc` (apt),
> and **recovered `.git` by re-cloning origin (which still serves the base `48d3074`) then fetching
> `handoff/sparq-wo020-inc1.bundle`** — the bundle discipline is what made the INC1 history
> recoverable after the snapshot dropped `.git` (and one working-tree file, `modules/out/main/`,
> restored from HEAD). The resume recipe below re-installs both. **Commit + bundle every increment:**
> the bundle in `handoff/` is the durable artefact, not `.git`.

---

## Delivery — 2026-10-06 (handoff prep: the pack, the run sheet, the closing bundle)

Operator instruction: **"prep handoff and sync zip."** The device delivery for INC1–INC4, in the
r8/rev-4 house shape:

| artefact | what |
|---|---|
| `sparq-update-2026-10-06-wo020-inc4.zip` (workspace root, one level above the repo; copy committed under `handoff/`) | **FULL-TREE pack, 471 entries** (defect #94's remedy: overlays any tree at or after the r8 pack / git `96fe57d`). Exclusions by the standing rule: `.git/`, `target/`, `Cargo.lock`, `logs/`, `*.wav`, `handoff/`, and **`SYNC-STAMP.txt` — the stamp does not ride and did not move from the sandbox** (MUST-NOT-MOVE honoured; the device re-stamps `--sync sparq-wo020-inc4-2026-10-06`, run sheet §1 step 2). sha256 + size: `handoff/sha256sums-wo020.txt` + the delivery message. |
| `WO020-INC4-RUN-SHEET.md` (ships inside the pack) | the operator's steps: apply → re-stamp → `gates.bat` (the ONE expected red: `design conformance` on the pre-existing §18 mockup set, 14 groups / 298 occurrences — on main before this pack) → the WO-020 device column (streams 23, guest tests + the budget block **exact-match** — deterministic replay, `mod validate` PARTIAL by design, the at-rest render via the `SPARQ_ATREST` env door, the shell visual row, the audit) → send-back list → what INC5 is. Both honest refusals are stated up front: validate's stages 3/4/6 and **PLAY #58** (until INC5's loader). |
| `handoff/make_pack_wo020.py` | the packer, checked in (the `make_pack.py` precedent): a **self-verifying pre-pack gate** — every entry's zipped bytes re-read and hashed against the working tree, the tree required clean vs HEAD, entry/namelist counts asserted against the numbers printed in the run sheet + SYNC.md (471 / 95 / 85 — the refusal it fired when the first artefact commit moved the namelist under the documents is the gate working, recorded in the delivery message), the MUST-NOT-MOVE surfaces (WIT, `modules/`, the stamp, `reference/fixtures`, the goldens) diffed vs base, and the exclusion rules asserted absent. It refuses to write a pack whose documents would lie about it — LATER.md's pre-pack-gate ideal, discharged for this delivery. |
| `handoff/sparq-wo020-delivery.bundle` (+ `.sha256`) | the closing bundle `96fe57d..HEAD` at the delivery seal — one fetch restores the whole WO-020 branch (INC1–INC4 + the delivery documents) into any clone of the base; supersedes the per-increment bundles for recovery (they stay committed as the increment record). |
| `SYNC.md` | new top entry (this pack: provenance, gates, the 90-path namelist vs `96fe57d`, THE ASK); rev 4 demoted to "Previous bundle" per the house pattern. |

**Windows friction found by the delivery prep (recorded, not papered over; `LATER.md` INC5 doors):**
the two at-rest cache-dir defaults disagree on Windows — the harness's `atrest_path()` writes
`%USERPROFILE%\.cache\sparq\at-rest\` while the app's `atrest_dir()` reads `%APPDATA%\sparq\at-rest\`
(the app grew the `cfg!(windows)` branch, the harness did not). Both sides honour `SPARQ_ATREST`
first, so the run sheet uses the env door; INC5 aligns the harness. Same section: the guest
workspace's `Cargo.lock` is untracked by the house `.gitignore`, so a device-side component REBUILD
resolves `wit-bindgen` unpinned — the shipped component is byte-pinned in the pack regardless.

**Branch state at the delivery seal:** `wo020-observatory`, base main `96fe57d` (= `origin/main`);
16 increment commits (`cef2385`..`44d623b`) + the delivery commits (the documents at `097a7ff`,
then the artefacts); working tree clean; nothing pushed (no credentials —
the bundles + this pack are the delivery, ROUND8's discipline).

---

## INC6 — "the live instrument" (plan of record `WO020-INC6-PLAN.md`) — **slices S1–S5 CLOSED in the sandbox (S4's wasmtime half device-first-compile; test008.bat is the run sheet); the DEVICE round outstanding**

### Slice S1 — the resizable instrument card (O-1/D15) — session 9, 2026-10-08, MEASURED

**Provenance / recovery note (read this first):** the sandbox was wiped, as expected. The
recovery DIFFERED from RESUME §4's recipe, for a recorded reason: neither the r8 pack
(`sparq-update-2026-10-07-wo020-inc5r8.zip` — a workspace-root artefact, one level above the
repo) nor `handoff/sparq-wo020-inc5r8.bundle` (never committed to any Fresh-start history; no
GitHub releases exist) is reachable from a fresh clone. But origin/main had ADVANCED past the
published `7ed2e08`: **`b2445d6` (2026-10-07 21:25) is the operator's newest "Fresh start —
rebuilt git history" re-publish, and its tree IS the r8 seal.** Verified by content, not hope:
RESUME.md is the r8 text, `WO020-INC6-PLAN.md` carries the four rulings, `sync_check` fails on
EXACTLY ONE file — `scripts/test007.bat`, disk 6390 B vs stamp 6308 B — which is r8's own
documented delta ("no code behaviour change beyond test007.bat's echo words"; the stamp keeps
its r7 hash by the never-rides rule, the device re-stamps), and `Cargo.lock` is absent
(gitignored; it rode the zip only — cargo regenerated it fresh, gates green). This session
therefore worked directly on `b2445d6`; the zip-extract + bundle ff-merge to `5f40436` is
subsumed by the re-publish (same tree, squashed history). **Bundle prerequisites are now
`b2445d6`.**

**What landed (every number below MEASURED in this sandbox):**

* `Node.size: Option<Vec2>` — the instrument card's display-BAND override, world px; `None` =
  the layout's default for the spec. The field holds the BAND (the face); the card is band +
  the D6 chrome — the only reading consistent with D15's own clamp numbers (≤ the declared
  `min_size` 2176×1120, which IS the face) and O-1's default (the 1088×560 BAND).
* The clamp lives in the op constructor (`Graph::op_resize_node`, the `op_set_param`
  discipline): ≥ `INSTRUMENT_BAND_MIN` 480×248 (D15's floor; a hypothetical smaller face
  clamps to its own face — the declared maximum always governs), ≤ the declared face (O-1:
  the guest never renders above it); a non-finite component sanitises to the default band
  (the `WireTrim` discipline); a request whose EFFECTIVE band equals the current one records
  nothing (the `op_move_node` discipline).
* `Op::ResizeNode { id, from, to }` — data, inverse, label ("resize"), `apply` arm;
  `note_patch` classifies it SILENT (the engine cannot hear a band size; S4's display pacer
  reads `Node.size` per frame — D17c/D19 — and forces a draw on resize, never via the audio
  ledger).
* The half-face default (O-1): `NodeSpec::instrument_band_default()` = face ÷ 2 → the
  Observatory's default card is **1120×728** (band **1088×560**); `layout::node_size(spec)`
  keeps its signature (the spec's default card) and the new `layout::node_card_size(&Node)`
  reads the override — FIT, FOCUS, the marquee and the layout pass all read the resized card
  "like any card" (D15).
* The corner handle: `NodeLayout.resize_corner` (the card's bottom-right, screen px,
  instruments only, selection-independent geometry); `layout::resize_handle_at` offers the
  grab ONLY while the node is selected, never at Dot LOD, never under a later-drawn card
  (COVERED IS UNTOUCHABLE — the hit-test's own ruling); the capture is
  `RESIZE_CAPTURE_RADIUS` = 36 screen px = half the class-L token, so the registered box
  measures exactly the 72 px floor (the port-capture idiom — zoom-invariant — at the handle's
  ruled class). Painted as a filled corner triangle + two grip hairlines in tokens only (R6),
  registered `canvas/resize/{id}` class L.
* The gesture: `Interaction::Resize` — press on the handle of the SELECTED card (ranked above
  the body's move-drag: a drawn handle beats the furniture under it, the cable node's
  ranking); the band follows the finger LIVE, clamped every update (no mid-drag ops); the
  release snaps to `LAYOUT_CANVAS_SNAP` (8 px — the grid and the window's edges commute:
  480/248/2176/1120 are all on-grid) and commits EXACTLY ONE `Op::ResizeNode` carrying the
  gesture's ORIGINAL `from` (the cable-node precedent: one undo per gesture, undo returns the
  size the finger found); a cancel restores; a release at the effective start records nothing;
  a LOCKED card refuses in words with the remedy (the lock's existing rule, applied to the new
  edit surface — recorded as a deviation below).
* `ShellUi::audit_element(id)` — the audit-side twin of `rect_of` (which reads the activation
  registry): the smokes now prove a command surface's registration BY IDENTITY (declared class
  + measured rect), not by a count delta.
* `docs/formats/project.md`: the reserved optional `size = [w, h]` node line, cut NOW so the
  container format lands with the door already open (D15's own words).
* The audit rows re-cut for O-1 (`run_instrument_smokes`): the default-card row (1120×728,
  the half face); the declared-maximum row KEEPS the D6/R3 wall-class coverage rule at the top
  of the ruled window (2208×1288 = 97.5% of the 2560 reference canvas, the FIT ≈ 0.98 note
  kept); O-1's fit row (the default card fits the 1080p chrome-only canvas rect 1624×740 at
  zoom 1; FOCUS ≈ 0.95, Full LOD; the 16-cell wall reads 133 px cells at FOCUS and 62 px at
  the ruled floor — both ≥ the 44 px touch floor: the floor's "words, not data" rationale,
  measured); smokes **55/56** drive the corner drag (live resize → 1208×624 snapped → ONE undo
  entry → the three-finger restore) and SPAWN+FOCUS through the REAL recogniser.

**Gates (measured, this box, after the slice):** fmt CLEAN; clippy `-D warnings` CLEAN
(workspace, `-p sparq-app --features ui`, `-p sparq-streams --features streams-net`,
`-p sparq-app --features streams,streams-net`); root `cargo test --workspace` **1047 / 0**
(the r8 baseline 1034 + 13 new S1 tests); `-p sparq-app --features ui` **45 / 1** — the 1 is
THE known sandbox-only red (`ui::live::…level_follows`, fails identically on the pristine tree,
RESUME §4); streams+streams-net **96 / 0 / 1-ignored**; the observatory workspace **124 / 0**
(goldens 6/6 inside), its clippy/fmt-per-package/`gen_manifest --check`/`make_coastline
--check` CLEAN; `sparq ui --audit` **PASS (0 failures)** — 71 PASS rows (was 64); the python
gates green except `token_audit`'s standing §18-mockup red (**14 groups / 298 occurrences — the
identical set to the baseline, zero new** — R6 scanned the new painter code clean); `mod
validate` PARTIAL by design. The `clippy -p sparq-app --features bootstrap-audio` cell is
ENVIRONMENT-BLOCKED in this box (`alsa-sys` needs the system ALSA headers; apt hangs) —
pre-existing, a device-side cell, untouched by this slice.

**Proven failable (a gate nobody has seen fail is a gate nobody trusts):** three injections,
each failed exactly its gate, each reverted BYTE-IDENTICAL (`cmp` after restore): (A) the
release skips the grid snap → the interact test fails with 1388 vs 1392; (C) the handle's audit
registration removed → the class-L row FAILs; (D) the default band set to the FULL face → five
audit FAILs (the O-1 card, both band rows, the fit row, smokes 55+56).

**Recorded deviations / judgment calls (no NEW design movement; all inside O-1..O-4 + D15):**

1. The targeted `#[allow(clippy::large_enum_variant)]` on `ConnectOutcome`, with measured
   provenance in its comment: the ruled `Node.size` grew `Node` **184 → 200 B** (both measured
   on pristine vs sliced trees), moving `Connected`-vs-`Refused` from 187 to **203** — three
   bytes past the lint's 200. Boxing the `Op` (clippy's hint) would re-shape a public API
   across three crates and ~12 call sites for a gesture-rate control-plane enum; clippy.toml
   itself names the targeted-allow-with-comment as the house pattern.
2. `Node.size` carries the BAND size (see above) — an interpretation of D15's wording forced
   by its own numbers, recorded here so the operator can overrule it if the intent was the
   card's outer dims (nothing else in the slice depends on the reading).
3. The locked-card refusal extends the lock's existing words to the new surface (the plan is
   silent; the canvas's lock semantics are not).
4. The toolchain recipe needed two sandbox fixes (folded into RESUME §4 for the next session):
   the zigcc shim must DROP cc-rs's `--target=x86_64-unknown-linux-gnu` (zig 0.17 refuses the
   vendor spelling — `ring` fails to parse it), and `ring`'s build needs `ar`/`ranlib` — `zig
   ar`/`zig ranlib` shims on PATH fix both. With those, `streams-net` (ureq+rustls+ring) builds
   and tests green here.

**Seal:** code commit + documents commit + `handoff/sparq-wo020-inc6s1.bundle` (+ `.sha256`),
prerequisite `b2445d6`. Recovery of the seal was PROVED this session: a fresh clone at
`b2445d6` + `git fetch handoff/sparq-wo020-inc6s1.bundle main && git merge --ff-only FETCH_HEAD`
lands the slice byte-exact (sha256 verified).

### Slice S2 — the typed panel band (D16) — session 9, 2026-10-08, MEASURED

**What landed (every number MEASURED in this sandbox):**

* `InstrumentDisplay.widgets: Vec<PanelWidget>` — the manifest's `[[ui.panel.widgets]]` TYPED
  (D16: "NodeSpec::display grows the widget list; today it keeps only the row COUNT", which
  stays for the band's height). `PanelWidgetKind` = the four declared spellings (`enum_select`
  / `toggle` / `slider` / `label`); an unseen kind is DROPPED at parse, never guessed at — the
  host paints no control it cannot route. Param ids and D7 `visible_if` gates resolve to
  param INDICES at parse; a control whose param does not resolve drops (a dead control is a
  lie with a hit-test); the row count still counts the manifest's declared rows, so the band's
  reservation never lies. The shipped manifest drops NOTHING: 27 widgets (18 enum_select — CELL
  + the sixteen gated STREAM pickers + LAYOUT — 5 toggles, 3 sliders, 1 status label).
* `layout`: `PanelWidgetLayout` rides the `NodeLayout` (one list the painter, the hit-test and
  the audit read — they cannot drift): the grid units map into the reserved band, rows on D16's
  44-px floor with the 16-px gaps (D6's formula, which the O-1 card numbers ride — UNMOVED),
  columns proportional (the Observatory's 17-unit row = 64-px columns at the default band);
  `visible` is the D7 gate evaluated against the node's CURRENT params every pass. `hit_test`
  routes a visible cell through the EXISTING `Hit::Param` door — enums open §8.4's picker,
  toggles flip, sliders map x — so every rule the card rows carry (tap-to-set, one undo per
  gesture, the binary flip, the picker's sheet, the live ledger) holds on the band UNCHANGED;
  a hidden cell falls through to the body (what is not drawn is not touchable); the label has
  no param and is never a hit. The `Interaction::Param` drag's track lookup grew the panel-
  widget arm (a band slider drag read no track before — found by the slice's own test).
* `canvas_ui`: the band paints its declared widgets at Full LOD (the Simplified contract is
  "no text" — the band sits reserved-but-empty there, exactly like the backbone param rows):
  enum chips with the chosen option's label + a down-chevron (a SHAPE, greyscale-safe), toggle
  chips with the param's id word + ON fill in the card's own class accent (fill AND word, never
  colour alone), sliders in the param row's track+knob idiom with name/value words, the label
  slot carrying the host's status words. The at-rest sentence (the plan's own words: "params
  edit; the wall re-renders live when the loader runs") rides the band's word strip whenever no
  provider sentence exists — refuse-in-words, never a frozen lie; the provider's sentence
  (S3/S4) replaces it the moment one exists. Audit registers every routed cell it draws under
  `canvas/panel/{node}/{widget}` at class S (the band's GIVEN floor is D16's 44 px) with the
  param-row dense-exception gate on the measured size.
* `inspector::value_text` grew the Enum branch: an enum READS as its chosen option's label on
  every surface (the band's chips, the inspector's row, one vocabulary with the picker's rows).
  Text/Blob keep their "· v1" refuse-only words.
* **Defect #100 found and fixed** (the ledger row carries the full record): manifest-parsed
  enums arrived with a ZEROED range, so every picker selection clamped to "no change" — the
  discovery door never caught up with §8.4 making enums settable. An enum's domain is now its
  option-index range (`0..=options.len()−1`), regression-pinned on the shipped manifest
  (selected_cell 0..=15, layout 0..=4, its default 4 in-domain) + the band's picker-route test
  + smoke 57. No device impact yet: nothing on SATURN routes a manifest-parsed enum through an
  editor until INC6 lands there.

**Gates (measured, after the slice):** fmt CLEAN; clippy `-D warnings` CLEAN (workspace, ui,
streams-net, app-streams cells); root `cargo test --workspace` **1054 / 0** (S1's 1047 + 7:
2 model, 1 layout, 3 interact, 1 inspector); `-p sparq-app --features ui` **45 / 1** (THE known
env red only); streams **96 / 0**; observatory **124 / 0** + clippy/fmt/gen_manifest CLEAN;
`sparq ui --audit` **PASS (0 failures), 75 rows** (was 71: the typed-band row + smoke 57's four
checks — registration-exactness "nothing undeclared exists anywhere", the SOLO flip through the
recogniser, the CELL chip → picker → row-select → the D7 re-gate the same frame, and the
PAINTED proof: the toggles' words + the CELL/LAYOUT readings + the at-rest sentence ride the
frame's text shapes — the same substance `--svg-out` serialises); python gates green except
`token_audit`'s standing mockup red (14 groups / 298, the identical set — R6 scanned the new
painter code clean). The convergence sheet (`--svg-out`) renders the wall at the sheet's fit
zoom, BELOW Full LOD — the band sits reserved-but-empty there BY DESIGN (S5 re-shoots the sheet
with the live wall); the painted proof at Full LOD is smoke 57's text check.

**Proven failable (two more injections, each reverted BYTE-IDENTICAL):** (G) the D7 gate forced
always-visible → 3 audit FAILs (the typed-band row 27≠12, the registration-exactness row, the
CELL re-gate smoke); (H) defect #100's `max: 0.0` restored → 3 unit tests + 1 audit smoke FAIL.

**Recorded judgment calls (no NEW design movement; all inside D16):**

1. The manifest declares its widgets `touch_class = "M"` (56 px); the band the host GIVES is
   D16's 44-px floor (D6's formula — the O-1 card numbers ride it, moving it is an operator
   ruling). The registration therefore measures class S (what is GIVEN, ≥ the 44-px contract
   minimum); the declared M rides along in the typed data, unre-read and unre-acted. If the
   operator wants M-sized band rows, that is a D6 movement (rows × 56 + gaps → the card grows
   24 px per row) — flagged here for the S5 convergence re-shoot, NOT decided here.
2. An unseen widget `kind` (e.g. `button`, which D16's prose mentions but no package declares
   and no semantics are ruled for) drops at parse rather than inventing a routing — refuse in
   words lives in the validator's report; the host stays honest by absence.
3. `value_text`'s Enum branch changes the INSPECTOR's enum-row word too (was "Enum · v1", now
   the chosen label) — the same vocabulary fix on every surface, pinned by its own test; no
   test anywhere asserted the old word.

**Seal:** code commit + documents commit + `handoff/sparq-wo020-inc6s2.bundle` (+ `.sha256`),
prerequisite `b2445d6` (carries S1's commits too — one fetch restores the whole INC6 branch so
far). 

### Slice S3 — the overrides store + the STREAMS dock tab (O-2/O-3/O-4, D18/D20) — session 9, 2026-10-08, MEASURED (the sandbox half; the device's live rows ride S4/the run sheet)

**What landed:**

* `sparq-streams/src/store.rs` (new, `streams`-gated, zero new dependencies — the house TOML
  subset parser): `Overrides` — per stream id an optional `key` and an optional `cadence_s`,
  in `sparq/streams.d/overrides.toml` under the OS user-data dir (D18's location, beside the
  streams cache; `SPARQ_STREAMS_OVERRIDES` is the env door for tests/review, the cache's own
  idiom). An array-of-tables file (dotted ids never ride table keys — the subset parser's
  limit, and the shape diffs like the graph part of a project: rows in id order, fields in a
  fixed order). The rules, each a tested door:
  — **the floor refuses in words** (O-3): `set_cadence(< 10)` answers the refusal sentence and
  KEEPS the old value; a hand-edited below-floor file row loads as no override (the floor
  governs the file too);
  — **env WINS over the file** (O-4): `env_closure[_over]` composes real-env-then-store, so the
  fetch's `{KEY}` fill and its EXISTING redaction ride along unchanged — a resolved key exists
  only inside `Request::url` wherever it came from;
  — **the value has no reading door**: the key field is private; out go only the mask
  (`••••last4`, D20), the state words (`SET (env)` / `SET (file)` / `UNSET` / `KEY NEEDED` /
  `—`, D18's exact vocabulary) and the closure; `Debug` is hand-implemented to mask (a `{:?}`
  in a log line is exactly the leak D18 forbids);
  — a malformed file is an EMPTY store WITH words (§6.3: the app never dies because a data file
  died); the save is tmp+rename+fsync (a half-written store is never observable);
  — **file keys resolve PER ENV VAR**: the three NASA streams share `SPARQ_NASA_API_KEY`, so a
  key typed on ANY of their rows serves all three (first row with a key in registry order
  governs — the closure's rule) and every sibling row shows the same honest state. One
  variable, one effective key; the store stays keyed per stream id because that is what the
  row you typed on shows.
* The broker reads through the store's data (D18's "`Broker::due` reads the override-then-
  registry cadence"): `Broker::set_cadence_override` (a SECOND refusing door below the floor —
  no door is silent), `effective_cadence`, and `window_for` — the stale horizon FOLLOWS the
  override (`Window::set_cadence`: a 10 s poller goes STALE at 30 s, honestly; the ruler moves,
  the data does not). The attempt-counting floor is untouched (the anti-thundering-herd rule).
* The CLI reads through the store at EVERY door (`list` — the effective cadence with its `*`
  and the registry value in a footnote, the D18 state words in the KEY column, the store's
  path + row count in the footer; `probe`, `probe --live`, `fetch`, `tail` — the composed
  closure, so a file key makes a stream fetchable exactly like an env key).
* The dock's STREAMS tab is ALIVE (O-2) under the `streams` feature (an honest disabled word
  without it): one row per registry stream — LED + word status (the provider's own derivation;
  with no driver thread the REPLAY provider's fixture statuses and the header says **`AT REST —
  fixtures`**, never a frozen lie — D20), the last-fetch age in words, the cadence (override
  bright, the registry value dimmed beside it; an editable field), the key state + KEY field
  (masked; an env-set key shows `SET (env)` and the field is DISABLED with those words — the
  precedence is visible, not a mystery; a disabled field stays registered so a tap REFUSES IN
  WORDS, and stays out of the audit — the menu-row convention). Rows scroll under the dock's
  own pan (the library's rule: a gesture over a surface belongs to that surface); fields are
  44-px class-S touch targets registered `dock/streams/{cadence,key}/{row}`; typing is the
  input-event feed idiom (the rename entry's), Enter commits through the store's doors, Escape
  cancels in words, and the cadence field pre-fills with the value it would replace — the
  first typed character REPLACES it (direct manipulation, no append-to-1800 surprise). The KEY
  echo is bullets — the mask is not only for the committed row.
* Smoke 58 (five checks, the recogniser for everything a finger does + the headless key doors
  for typing — the rename idiom): the tab opens and its rows/header are honest; 5 s refuses in
  words keeping 1800; 10 s is honoured (the file written, the row reading `10 s (registry
  1800 s)`, the file re-loading); the key commits MASKED — the sentinel rides the user-data
  file ALONE, never a log line, never a frame's texts (the redaction acceptance, grepped); the
  env-set key disables the field with its words and the tap refuses.

**Gates (measured, after the slice):** fmt CLEAN; clippy `-D warnings` CLEAN (workspace, ui,
**ui+streams**, streams-net, app streams+net cells); root `cargo test --workspace` **1055/0**
(+1: the window's cadence-ruler test; the store's 9 tests ride the `streams` feature);
`-p sparq-streams --features streams,streams-net` **106/0/1-ignored** (was 96; +9 store, +1
window); `-p sparq-app --features ui,streams` **47/1** (THE known env red only); `ui --audit`
**PASS** in BOTH feature faces — `ui` (75 rows) and `ui,streams` (**80 rows**: smoke 58's five
checks); the python gates unchanged-green (no tool, token or module surface moved); the
observatory workspace untouched (124/0 re-verified). The DPI-invariance matrix is untouched:
the tab opens on LOG by default, so the matrix shells register nothing new.

**Proven failable (three injections, each reverted BYTE-IDENTICAL):** (I1) the floor stops
refusing → the store unit test AND smoke 58's floor check FAIL; (I2) the committed key pushed
into the log → the redaction smoke FAILS (the grep is the gate); (I3) the precedence inverted
(file wins) → the env-wins unit test FAILS.

**The acceptance's DEVICE half, stated honestly:** "entering the NASA key flips neo/epic/power
from 429/DEMO_KEY to the key's rate" needs the live driver thread — that is S4's slice and the
run sheet's row; the sandbox proves the whole mechanism behind it (the closure resolves the
file key into the request, the redaction holds, the floor and the precedence behave) over the
mock transport + a temp-dir store, exactly as the plan scoped it.

**Recorded judgment calls (no NEW design movement):**

1. `fetch.rs`'s `KeySource::Env(var)` metadata keeps its vocabulary: the composed closure IS
   the env door (D18's own sentence — "the broker's env closure reads key-store-then-env"), so
   a file-resolved key records as the var it filled; the SOURCE visibility lives where D20 puts
   it (the tab's words, the list verb), and the value stays redacted either way.
2. The disabled (env-set) KEY field stays in the registry (a tap refuses in words — the
   menu-row convention, refuse-in-words over silent-inert) and out of the audit (a disabled
   control is not a touch target).
3. S4's seam, pre-cut: `shell.overrides` is the store the driver will hand the broker
   (`env_closure` + `set_cadence_override` at launch and on every edit); the tab already reads
   the provider trait, so the BrokerProvider swap changes the header word and nothing else.

**Seal:** code commit + documents commit + `handoff/sparq-wo020-inc6s3.bundle` (+ `.sha256`),
prerequisite `b2445d6` (carries S1+S2 too). 

### Slice S4 — the live plane's host-side seams (D17/D19) — session 9, 2026-10-08, MEASURED (sandbox half); the wasmtime half is DEVICE-FIRST-COMPILE, `test008.bat` is its run sheet

**What landed:**

* **The D19 token, ruled into `layout.toml`**: `canvas.instrument_display_hz = 15` (the plan
  itself names the token, its value and its home — an additive token change, regenerated
  through `token_gen`, the bundle round-trip green, the guest's metrics mirror untouched (the
  guest never reads host pacing), no new `token_audit` violation).
* `ui/live_display.rs` (new; the seam module — toolkit-independent, rides `any(test, ui)`):
  the `LiveDisplay` trait (one draw → `Result<Vec<Item>, String>`, Err carrying §3's watchdog
  sentences), `DisplayFrame` (the band's ACTUAL px — O-1 reaches the guest; the camera's LOD in
  the contract's spelling; `time_sec` = the shell's animation clock, D8-legal), the
  `DisplayPacer` (the token's 66 ms period; `due`/`record`; an edit or an LOD change forces the
  next frame regardless of pace — D19; five CONSECUTIVE overruns bypass, `BYPASS_AFTER = 5`,
  a success resets the count; a bypassed instance is never due again until an edit re-arms it —
  the retry door, recorded below), `BYPASS_WORDS` (§3's sentence, pinned by a test: "DISPLAY
  BYPASSED — five consecutive draws overran; the last render stands (the audio instance is
  untouched)"), and `LiveSurfaces` (the band switch's LIVE shelf, keyed by module id — the
  at-rest store's own key; "live where the instance carries it, at rest otherwise", the
  meters' rule extended).
* The shell's tick (`tick_live_displays`, every frame after the canvas layout is final):
  `pending_displays` (the launch shelf, keyed by module id) attaches an instance to its
  module's FIRST canvas node with words; the edit diff (params + size per node) and the LOD
  transition feed the pacer's force; draws land on the live shelf with the band's px; failures
  skip + count + log the watchdog sentence, and the fifth bypasses — the shelf drops (the band
  falls back to at rest) and the words go to the log. Nodes that leave the canvas drop their
  instance, pace and memo (no ghosts).
* The band switch in the painter (`draw_instrument_display`): live shelf first, at-rest second,
  BYPASS words over both; the word strip's precedence is a fact ordering — the watchdog's
  sentence beats the provider's, which beats the at-rest sentence. A live surface was drawn at
  the band's actual px, so its scale is exactly z (native text, O-1); the at-rest render keeps
  its uniform scale-to-fit.
* `ui/streams_driver.rs` (new, `streams`-gated): the driver — `poll_once(now)` is the WHOLE
  loop body (due → poll → the outcomes' words stay in the broker), pure over the injected
  clock/transport/sleeper, so the sandbox proves it with the mock (D11's firewall, on the
  driver itself); `start()` is the thin device thread (`loop { poll_once(now_unix()); condvar
  wait }` — the wait is a control-plane condvar timeout, never the banned audio sleep; `wake()`
  cuts the wait short on a cadence/key edit; `stop()` + the shell's `Drop` joins it — a leaked
  poller would keep hammering rate-limited feeds). The env door is the store-composed closure
  (O-4: env wins; the file key reaches `Request::url` and its existing redaction unchanged);
  the launch applies the store's cadence overrides to the broker (D18's "loaded at launch"),
  and `apply_cadence` re-applies + wakes on every edit (the shell's commit path calls it).
* The provider swap (D17b): the shell grew `broker_provider` — the STREAMS tab and the §5.2
  status label read the LIVE mirror first (`LIVE — the driver thread polls on cadence` header)
  and the replay fixtures second (`AT REST — fixtures`), through the StreamProvider trait's
  own door (one trait, two guests — INC5's promise, now exercised). `start_live_streams()` is
  an EXPLICIT launch door the WINDOW host calls — never `new()`: the headless audit shares the
  constructor and must stay hermetic even in a `streams-net` build (a gate that spends the
  operator's API quota per run is a gate nobody trusts — recorded as a judgment call).
* `ui/instrument_launch.rs` (new, **DEVICE-FIRST-COMPILE** — `all(ui, instrument-host)`; the
  §8 wall stands, this cell cannot compile in the 1 GB sandbox): the launch registration
  (D17a) — the engine, the provider precedence (broker → fixtures → REFUSE in words: a
  sources-blind instrument would draw holes as data), and per discovered loadable package ONE
  launch load whose split feeds both halves: the display instance parks on the pending shelf
  (`RuntimeDisplay` — the `budget_call(…, call_draw)` idiom of stages.rs verbatim, the WIT
  surface through D13's interchange into the SAME parser the at-rest shelf reads), and the
  audio half registers through the registry's `register_instrument` factory door — the factory
  RELOADS per instantiation (a wasm `Store` has one owner and a factory is
  `Fn() -> Box<dyn Module>`), with a `LoadRefused` failure module (Failed blocks + silence
  substituted, the §6.3 shape — never a stub that pretends to make sound). PLAY's #58 refusal
  lifts STRUCTURALLY: the registry has the module, so the canvas and the executor agree.
  Desk-checked call by call against the vendored API (16 surfaces verified:
  `InstrumentRuntime::new`, `load_package`, `LoadContext`'s six fields,
  `LoadedInstrument::split`, `budget_call`, the `FrameContext` field-for-field mirror of
  stages.rs, `surface_to_ir_json`/`ir_json_pretty`, `register_instrument`'s probe semantics,
  the `Module` trait's full member list, …); its first compile is on SATURN and
  **`scripts/test008.bat` is the run sheet** (stamp → build → the launch words → PLAY / the
  cells update / the ticker scrolls / PAUSE freezes / a cell swap re-renders within a frame →
  the watchdog INJECTION on a copy with `max_fuel = 100000` → the STREAMS tab's LIVE rows +
  the NASA key flip (S3's device half) + the on-device redaction findstr → the send-back list;
  DO NOT CANCEL — session 8's evidence rides in its header).
* Headless smoke 59 (four checks): the attach + the 15 Hz pace (66 ms — three frames inside
  the period draw NOTHING, the frame past it draws); the live shelf carries the band's ACTUAL
  px and a RESIZE reaches the guest (1088×560 → 2176×1120, O-1 end to end); an edit forces the
  next frame; five consecutive scripted overruns (the mock's Err carries the fuel sentence)
  bypass with §3's words, the shelf drops, and no further draw is attempted. The scripted
  instance's state rides ATOMICS — `Rc` and `Mutex` are both clippy.toml-disallowed types, and
  a smoke is not exempt from the house it measures.

**Gates (measured, after the slice):** fmt CLEAN; clippy `-D warnings` CLEAN on SIX cells
(workspace, app `ui`, app `ui,streams`, streams `streams-net`, app `streams,streams-net`, app
`ui-window,streams,streams-net` — the last is the window host's real feature set, minus
`instrument-host` which is the device's); root `cargo test --workspace` **1061 / 0** (the r8
baseline 1034 → +27 across S1–S4); `-p sparq-app --features ui,streams` **56 / 1** (THE known
env red only — that cell also carries the driver's 3 hermetic tests and the seam's 7);
streams(+net) **106 / 0 / 1-ignored**; `ui --audit` **PASS (0 failures)** in both faces — `ui`
79 rows, `ui,streams` **84 rows** (smoke 59's four); the python gates green incl.
`check_text_io` (which caught test008.bat's unescaped echo parens — defect #69's gate earning
its keep; fixed + re-verified) and `token_gen --check` deterministic after the D19 token;
`token_audit`'s standing mockup red unchanged (14 groups / 298, zero new); the observatory
workspace **124 / 0** + goldens 6/6, `gen_manifest`/`make_coastline` current.

**Proven failable (three injections, each reverted BYTE-IDENTICAL):** (J1) the pacer's `due`
forced true → the pacing check FAILs (and the bypass check with it); (J2) `BYPASS_AFTER` 5→50
→ the bypass check FAILs; (J3) the driver's env door skips the store → the driver test's "the
file key reached the request URL" FAILs.

**Recorded judgment calls (no NEW design movement; all inside D17/D19):**

1. The bypass's re-arm door is the EDIT (D19's force doubles as the retry: a param edit, cell
   swap, resize or LOD change clears the bypass and the count). The plan names no other door;
   an edit is the user saying "try again", and a bypass with no retry would need a shell
   restart — a worse lie.
2. `start_live_streams` is the window host's explicit door, not `new()` — the headless audit's
   hermeticity outranks the launch convenience (recorded above; test008's [B]/[E] rows are the
   live proof).
3. One display instance per MODULE (the pending shelf's key): a second node of the same module
   stays at rest, in words — a wasm `Store` has one owner and instance cloning is not a v0
   door. The Observatory ships as one card; the limit is recorded, not hidden.
4. The factory RELOADS per instantiation (the launch load's audio half is the load's proof and
   is dropped): §2.2's pair is per-load, the registry's factory is context-free, and a pooled
   instance would be a second owner — the cost is measured on the device (test008 [B]'s rows),
   not hidden.
5. The tick's edit-diff reads the GRAPH (params + size per instrument node per frame, ~26
   floats while instances exist): the force cannot be forgotten by a future edit door — the
   live-sync ledger (`take_patch_changes`) was NOT touched, it belongs to the audio session.
6. Near-miss, recorded per the house-incident idiom (no trace in the tree): a debug scratch
   file was created under `crates/sparq-app/examples/` and removed with `rm -rf` on the
   directory — taking the checked-in `probe_alloc.rs` (the allocation gate's example) with it.
   Caught by `git status` before any commit, restored byte-exact from HEAD, the gate re-run
   green. The lesson joins the house notes: scratch files live in /tmp, never in a repo
   directory, and `git status` after any accident.

**Sandbox-vs-device split, stated honestly:** everything above the trait line is MEASURED here
(the pacer, the shelf, the bypass, the tick, the driver's loop body, the provider swap, the
band switch, the store composition). Everything behind it — the wasmtime loader at launch, the
real guest draws, the real driver thread on the network, PLAY with the Observatory — is the
device's, exactly like INC5's loader was: written against the vendored API, desk-checked, and
`test008.bat` records it. **The S4 acceptance rows ([B1]–[B4], [D], [E1]–[E7]) are
DEVICE-PENDING and ride the next run-sheet round** (with session 8's still-outstanding gates.bat
re-run and [G]/[I] eyes).

**Seal:** code commit + documents commit + `handoff/sparq-wo020-inc6s4.bundle` (+ `.sha256`),
prerequisite `b2445d6` (carries S1+S2+S3 too).

### Slice S5 — the card's RATE row + convergence (D20's second door) — session 9, 2026-10-08, MEASURED

**What landed:**

* **The host RATE row** (D16's furniture clause, D20's second door): a 40-px row under the
  guest's display band, gutter-inset — the `out/main` driver-info band's own token and
  placement precedent ("host words in the card, never guest widgets"). The layout reserves it
  on EVERY instrument card in every build (`NodeLayout.rate_band` + `rate_field` — geometry in
  the layout, so the hit-test and the painter read one rect); what fills it is the shell's
  per-frame data, and a build without the stream plane shows the reserved row with honest
  words ("RATE — no stream plane in this build"), never a pretend field. The guest's display
  band STOPS where the row starts (the available-height formula grew the subtraction — the
  display never grows into the host's furniture).
* **The field writes the SAME override the STREAMS tab does** (one store, two doors): the tap
  routes as `Hit::RateField` → `CanvasEvent::RateField` (the RenderWav division of labour — the
  canvas routes, the SHELL acts, because the shell owns the store) → the streams entry
  (`StreamsFieldTarget::Card`), Enter commits through `Overrides::set_cadence` — the O-3 floor
  refuses in words and keeps the old value, the save + the driver's `apply_cadence`/`wake` ride
  along, and the log line names the stream and BOTH doors.
* **The words name the governed stream and the shared-feed truth** (the acceptance, verbatim):
  `RATE · CELL 07 → space.epic: 3600 s — 2 panel(s) ride this feed and share its rate (the
  broker's truth)`. The resolution reads the node's LIVE params per frame (selected_cell →
  cell_NN → the enum option's value IS the registry id — the generator's own vocabulary), so
  moving the CELL dropdown moves the governed stream THE SAME FRAME (smoke-proved); the shared
  count walks every cell param. The field shows the override bright with the registry value
  beside it, the tab's own shape; it registers in the audit with the card rows' dense-exception
  idiom (40 px world → badged under class S, unregistered under the 32-px floor, where the row
  is honestly a reading and the tab is the door).
* **Card numbers moved, band numbers did not** (recorded, not invented): the chrome grew
  168 → 208 px, so the default card is **1120×768** and the declared-maximum card **2208×1328**;
  the O-1 BAND numbers (1088×560 default, 2176×1120 maximum, the 480×248 floor) are untouched.
  S1's fit row was re-cut to O-1's OWN words — the BAND fits the 1624×740 canvas rect at zoom 1,
  FOCUS frames the CARD at 0.90 (Full LOD), the 16-cell wall reads 126 px cells at FOCUS and
  62 px at the ruled floor (both ≥ 44). The coverage rule (≥ 95% width at the 2560 reference)
  still passes at the maximum: 97.5%, width unchanged. The FIT note at the maximum reads ≈ 0.95.
* **The convergence sheet, re-shot** (sandbox half): `sparq ui --svg-out … --review --headless 6`
  with `ui,streams` — sha256 `56e446d3…`, 173 084 B, 6 frames, Full LOD. The sheet carries the
  whole INC6 face at a glance: the half-face wall card, the typed band (SOLO/FULL/TICKER/GRID/
  PAUSE chips, the CELL and LAYOUT enum readings "Cell 01"/"4 × 4 (16)"), the RATE row, and the
  provider's sentence "LIVE 16/16" from the fixtures. NOT checked in under `design/mockups/`:
  token_audit scans `*.svg` there and a shell-dumped SVG (resolved hex, like the §18 mockups)
  would move the standing-red baseline — the wo012 convergence precedent is a PNG, and the
  PNG re-shoot WITH THE LIVE WALL is the device round's (test008 [B]). The sandbox artefact is
  recorded by sha + content grep, here.
* **The mockup audit re-run** (the acceptance's own line): `token_audit` = 14 violation
  groups / 298 occurrences — the IDENTICAL standing set (§18 mockups, pre-WO-020); S5 added
  zero appearance literals (R6 scanned the new painter code clean).

**Gates (measured, after the slice):** fmt CLEAN; clippy `-D warnings` CLEAN on all SIX cells;
root `cargo test --workspace` **1062 / 0** (r8 baseline 1034 → +28 across S1–S5);
`-p sparq-app --features ui,streams` **56 / 1** (THE known env red only); streams(+net)
**106 / 0 / 1-ignored**; `ui --audit` **PASS (0 failures)** both faces — `ui` **80 rows**,
`ui,streams` **88 rows** (the RATE-geometry row + smoke 60's three checks: the words name the
governed stream + follow the CELL dropdown + count the shared feed; the field registers; the
field writes the tab's store and 5 s refuses in words); the python gates green; the observatory
workspace **124 / 0**, untouched.

**Proven failable (three injections, each reverted BYTE-IDENTICAL):** (K1) the RATE words stop
naming the governed stream → the words check FAILs; (K2) the card's stream resolution dies →
ALL THREE smoke-60 checks FAIL (no words, no registration, no store write); (K3) the card
formula drops the host row → the default-card row FAILs (1120×728 ≠ 768).

**Recorded judgment calls / events (no NEW design movement):**

1. **The transient, resolved:** one audit run mid-injection-sequence showed smoke 60's exact
   3-FAIL signature on what was believed a clean tree; K2, properly run later, reproduced that
   signature EXACTLY (the resolution-dead face). The pre-hardening smoke framed the card via
   FOCUS — a framing dependency with a Simplified-LOD failure mode (zoom < 0.6 → no rate
   surface at all → all three checks fail together). The smoke is now deterministic BY
   CONSTRUCTION: an explicit camera (zoom 0.8 — 40 px × 0.8 = the 32-px dense floor exactly,
   measured not assumed) and a measured Full-LOD precondition folded into the checks. Ten+
   consecutive greens before and after the hardening; the gate's failability is injection-proved
   three ways.
2. The RATE row exists (reserved, in words) in EVERY build — a feature-gated card geometry
   would make the audit's numbers feature-dependent, and the DPI-invariance matrix would
   measure two different shells. The words differ by build; the geometry does not.
3. Smoke 60 writes a real store file under `SPARQ_STREAMS_OVERRIDES` in the temp dir and
   removes it (smoke 58's idiom); the shell's own default-path store is never touched by the
   suite.
4. **S2's touch-class flag, carried to the operator** (NOT decided here): the manifest declares
   its widgets `touch_class = "M"` (56) while the band gives D16's 44-px floor and the audit
   registers what is GIVEN (class S, clean). The convergence re-shoot shows the row visually;
   an M-sized band is a D6 movement (card +24 px) and an operator ruling.

**INC6 status after S5: slices S1–S5 CLOSED in the sandbox.** Outstanding for the work order is
the DEVICE round: `test008.bat` (the S4 launch words, PLAY #58 gone, the living wall, the
watchdog injection, the STREAMS tab LIVE + the NASA-key flip = S3's device half, the on-device
redaction findstr), the convergence PNG re-shoot with the live wall, plus session 8's still-open
rows (the uninterrupted `gates.bat` re-run [K], the [G]/[I] eyes).

**Seal:** code commit + documents commit + `handoff/sparq-wo020-inc6s5.bundle` (+ `.sha256`),
prerequisite `b2445d6` (carries S1–S4 too). **Next action, anywhere: the DEVICE round —
`scripts/test007.bat`'s uninterrupted gates re-run + `scripts/test008.bat`, or operator
rulings on the carried flags (the M-vs-S touch class, the one-instance-per-module limit).**

---

## INC5 — the device increment: runtime loader + live acceptance — **IN PROGRESS (sandbox slices 1–2 CLOSED; the device run outstanding; INC5b's launch wiring is DISCHARGED by INC6 S4's `instrument_launch` — device-confirmation rides test008.bat, do not build it twice)**

**Session:** 2026-10-07, fresh sandbox on the re-published main `a9dbf31` (which carries
INC1–INC4). Branch `wo020-inc5`. Baseline re-measured EXACTLY at the INC4 seal (root 1033/0/1,
streams 75/0, observatory 124+6, every python gate) plus one pre-existing fmt red fixed as the
first hygiene commit (`577e13d`).

### The environment verdict, re-measured (the §8 discipline)

The plan called INC5 device-only; this session split it instead: **everything that does not need
cranelift is sandbox-buildable and was built, measured, committed.** The wasmtime half stays
device-first-compile, and the blockage was RE-PROBED, not assumed: a scratch winch-only tree
(wasmtime `=49.0.2`, `default-features=false`, `features=[std,runtime,winch,component-model]`)
still SIGKILLed `cranelift-codegen` at `cargo check` — `cargo tree -e features` shows why:
`wasmtime-internal-winch` → `wasmtime-internal-cranelift` → `cranelift-codegen`. **No wasmtime
feature set avoids cranelift; the 1 GB wall is categorical** (§8's conclusion, now from two
directions). The mitigation that replaced compiling: the pinned generator itself
(`wasmtime-internal-wit-bindgen 49.0.2`) is a standalone crate — a scratch driver
(`/tmp/bindgen-dump`, recipe: `Resolve::push_dir` → `select_world(&[pkg], Some("instrument"))` →
`Opts{with: {"sparq:instrument/assets.asset": AssetRep}}.generate`) **RAN the real bindgen over
the frozen WIT and dumped the exact 2677-line host expansion**; every wasmtime signature the
loader uses was then verified against the vendored sources (`Agent::new_with_config` was the one
guess the vendored read caught and killed). The loader is written against measured API, not
memory — but its first COMPILE is the device's / CI's feature-on cell.

### Delivery — 2026-10-07 (handoff prep: the pack, the run sheet, the closing bundle)

Operator instruction: **"Prep sync zip."** Delivered in the r8/rev-4 house shape:
`sparq-update-2026-10-07-wo020-inc5.zip` (**485 entries FULL-TREE**, 6 725 644 B, sha256
`d14db3698e4908c289532813abc7d44051e4452f4bf5323a4e4044a188b3cc88` — workspace root + the
committed copy under `handoff/` + `sha256sums-wo020-inc5.txt`), `WO020-INC5-RUN-SHEET.md` (ships
inside the pack: apply → re-stamp `sparq-wo020-inc5-2026-10-07` → gates → test007 A–L → the
send-back list, with BOTH honest states up front: INC5b/PLAY #58 and the pre-existing
token_audit red), the SYNC.md top entry (INC4 demoted to Previous), the packer adapted +
self-verified (its own clean-tree gate caught an uncommitted packer fix mid-run — the gate
working, recorded), and `handoff/sparq-wo020-inc5-delivery.bundle` (`a9dbf31`..the artefact
seal, sha `3a32f9b2…`) superseding the slice bundle for recovery. Namelist vs `a9dbf31`:
40 paths (17 A / 23 M), 37 shipped; MUST-NOT-MOVE surfaces (WIT, `modules/`, the stamp,
fixtures, goldens) asserted unmoved by the packer; nothing pushed (no credentials — the bundles
+ pack ARE the delivery). NOTE: this delivery record itself post-dates the bundle it names by
one commit (a bundle cannot carry its own commit — the r8 pattern); its sha is in the sums file
beside the pack.

**Incident (recovered, no trace in the tree):** between the seal turn and this one the sandbox
snapshot dropped `.git` AGAIN (the environment note's known behaviour) — and with it
`modules/out/main/sparqmod.toml` from the working tree (the #93 trap's directory-name pattern,
second occurrence). Recovery was exactly the recorded recipe: re-clone origin (still serving the
base `a9dbf31`), fetch `handoff/sparq-wo020-inc5.bundle`, `symbolic-ref` HEAD onto the branch +
`git reset` (tree untouched), `git checkout -- modules/out/main/sparqmod.toml`, re-commit the
bundle artefacts. The reset-vs-seal diff came back EMPTY except the known drop — byte-identical
recovery, proven not assumed. Lesson re-recorded: the bundle in `handoff/` remains the durable
artefact; commit + bundle every slice, exactly as this WO has.

### Device session 1 — 2026-10-07 (the first test007 run: REFUSED at [A], root-caused from the log)

The operator applied the pack and ran `test007.bat`; the symptom reported was "the new instrument
does not show in the library". The log is unambiguous: **[A] never built** — `sync_check: FAIL,
this tree is NOT sync sparq-wo018-rebuild-2026-10-05 (27 files differ)` → build.bat's defect-#78
guard refused (the re-stamp, run-sheet §1.2, had not run yet), so **[B] `unknown command
streams`**, **[D] `unknown command mod`**, and **[E] the shell was the STALE pre-WO-020 exe** —
a shell from before the instrument browser existed. Nothing was wrong with the pack or the
discovery path; the run-sheet's own ordering was skipped and my test007 [A] note ("stamp-mismatch
words are EXPECTED") was WRONG — build.bat refuses, it does not merely warn. Found-by-running-it
fixes, landed in-tree (rides the next pack; the deployed zip is unchanged):
* `test007.bat` grows **step [0]**: `sync_check` first, fail fast with the exact re-stamp command
  printed — the trap cannot re-fire silently.
* `WO020-INC5-RUN-SHEET.md` §3: "re-stamp BEFORE test007", with this incident named.
* The CWD-relative + silent discovery defect found while diagnosing (the shell says nothing when
  `instruments\` is absent from the process CWD) stays recorded in LATER.md as INC5b's first fix —
  it was NOT this symptom's cause, but it is real and measured (root CWD: observatory in the
  convergence SVG ×2; other CWD: ×0).
Remedy given to the operator: `python tools\sync_check.py --write --sync
sparq-wo020-inc5-2026-10-07` then re-run `scripts\test007.bat` (first build 10–40 min; then
[B]/[D]/[E] run against the new binary).

### Device session 2 — 2026-10-07 (the re-stamp run: wasmtime COMPILES on SATURN; the first-compile defects, found and fixed)

Re-stamp → test007 again. Step [A] built for real: **wasmtime 49.0.2 + cranelift compiled clean
on the device** (the §8 memory wall is the sandbox's alone, as ruled), and the build reached the
runtime — **5 errors + 1 warning, all in the blind-written half** (four `*display_w/h` derefs of
by-value f32s; a format! passing `draw_fuel` positionally AND inline — which also would have
printed draw fuel as the memory high-water; a dead `full_json` initializer under -D warnings;
two unused test imports; the preview assertion not knowing svg() opens with the XML declaration).
All fixed smallest-honest-patch (`1e3fe95`), the budget-overrun report lines moved to
E-CROSS-FIELD (measured-vs-declared, not requires-unsupported), and the layers rustc never
reached were desk-checked against the vendored sources in the same pass (HasSelf, BlockStatus's
module, the sparq-ui pub faces). **Revision pack `sparq-update-2026-10-07-wo020-inc5r2.zip`**
(485 entries, sha `d6e283231252626aca1954da2d40125b09ec3de1af404e7883f1b4d60ed2947c`, stamp
`sparq-wo020-inc5r2-2026-10-07` — a second zip must not share the first's stamp name); test007
grew the fail-fast step [0] after session 1. This is the §4 loop working as designed: the device
is where the instrument-host cell compiles; every defect it finds comes back, gets fixed, and
gets recorded.

### Device session 3 — 2026-10-07 (the caret bug: a fail-fast gate that refused everyone)

The operator applied r2, re-stamped (`sync_check: OK — 175 of 175 … byte-for-byte`) and test007
STILL refused at step [0]. Root cause, in words: my step-[0] patch wrote `if errorlevel 1 ^(` —
the ESCAPED open paren. cmd treats `^(` as a literal, the block never forms, and the "guarded"
body (`echo TEST007 REFUSED … exit /b 1`) runs UNCONDITIONALLY: the stamp gate refused every
tree, healthy ones included. One character class, found on the device because a bat file is only
really parsed by cmd. Fixed in r3; `check_text_io.py` grows **rule 5** (an escaped `^(` on a
structural if/for line is a defect — rule 4's mirror image), proven failable by injection then
reverted byte-identical, per the house rule that a gate nobody has seen fail is a gate nobody
trusts. Revision pack `sparq-update-2026-10-07-wo020-inc5r3.zip`, stamp
`sparq-wo020-inc5r3-2026-10-07` (a third zip must not share the second's stamp name either).

### Device session 4 — 2026-10-07 (the zero-fuel instantiation trap; the loader's first real run)

r3 on the device: the runtime COMPILED, linked, and ran — stages 3–6 all reported. The load
refused with `instantiate: wasm trap: all fuel consumed by WebAssembly`, and my error words
mis-blamed the import list. The real defect, one line: **`consume_fuel(true)` starts the store
at ZERO fuel and instantiation itself executes wasm** (start sections, lowering trampolines) —
`spawn()` fueled the store only inside `budget_call`, which first runs AFTER instantiate. Fixed:
the store is fueled with the prepare-class budget before instantiation; the instantiate error
classifies through `trap_words` (traps keep their own words; only NON-trap failures carry the
capability-by-absence sentence — session 4's lesson: do not blame imports for a fuel
exhaustion); and instantiation fuel is now recorded in the stage-5 report (an invisible load
cost surprises someone later). Also in r4: the stages' draw calls run under a DIAGNOSTIC budget
(prepare's multiplier) and stage 5 JUDGES the measured number against the declared `max_fuel` —
a genuinely under-declared guest now produces a measured re-baselining fact instead of an
unmeasured trap (the Observatory declares 2 000 000; its real full-wall draw cost was never
measured anywhere — the sandbox has no runtime, and the harness is fuel-free). Production draw
enforcement stays the watchdog's row (§3): declared budget, frame-skip, words. The desk check
missed the zero-fuel instantiation because wasmtime's fuel semantics for `instantiate` were
never verified against the vendored source — recorded as the class it is: an API-behaviour
assumption, exactly what the device loop exists to catch. Revision pack
`sparq-update-2026-10-07-wo020-inc5r4.zip`, stamp `sparq-wo020-inc5r4-2026-10-07`.

### Device session 5 — 2026-10-07 (the epoch deadline that was never armed; [B] live-probed green)

r4 on the device: the fuel fix landed (instantiation now runs metered, budget visible in the
words), the load reached wasm — and trapped `epoch deadline exceeded` at instantiation. The
vendored doc says it outright: `epoch_deadline_trap()` arms the trap but leaves the deadline at
ZERO — "it's required to call `Store::set_epoch_deadline` or otherwise wasm will always
immediately trap." One line fixed (`set_epoch_deadline(EPOCH_DEADLINE_TICKS)`, 1 000 ticks — the
gates have no incrementer; the production watchdog's cadence lands with INC5b and turns ticks
into the §3 deadline). Session 5 also MEASURED the stream plane's device half for the first
time: **[B] the live probe ran green from SATURN — 22× HTTP 200 + FIRMS KEY NEEDED in words**
(the §3.3 asterisks are device facts now). r5: `sparq-update-2026-10-07-wo020-inc5r5.zip`, stamp
`sparq-wo020-inc5r5-2026-10-07`.

### Device session 6 — 2026-10-07 (id() is not cheap: the first call pays the guest's lazy init)

r5 on the device: instantiation SUCCEEDED (epoch armed, fuel metered, instantiation cost
recorded) and the load reached the first guest call — `guest.id()` trapped, `fuel budget
exhausted (2000000 of 2000000 burned)`. Two facts, one fix: the SDK builds its state on the
FIRST export call (for the Observatory that includes `Wall::new()` parsing the embedded
coastline), and the loader had given a load-time lifecycle call the PER-BLOCK budget. The
per-block budget is `process`'s contract and nothing else's: `id`/`activate`/`deactivate`/
`save-state` now ride the control budget (prepare's multiplier, 64×). Recorded for the guide's
eventual author-facing text: a guest's first-call cost is real and lands on whichever door opens
first. r6: `sparq-update-2026-10-07-wo020-inc5r6.zip`, stamp `sparq-wo020-inc5r6-2026-10-07`.

### Device session 7 — 2026-10-07 (the operator's zoom report: three defects, all root-caused, all fixed in-tree; the max_fuel re-baseline ruled)

The operator reported "**sparq becomes unstable when the zoom buttons are used while the
observatory is loaded**" and sent the [D] gate log (stages 4+5 FAIL). Three distinct defects,
each proven before its patch — none of them the class the gate words first named:

* **#97 — the zoom instability itself (the report): `dash_segments`'s f32 walk stalls at EVERY
  toolbar zoom but exactly 1.0.** The egui display-list painter decomposes dashed/dotted strokes
  into segments (the SVG back end rides `stroke-dasharray` — that is why no headless gate ever saw
  it). In f32 the per-step drift lands `phase` within an ulp of a boundary (`on*z` and `(on+off)*z`
  are different roundings of the same products), `at + (on - phase)` rounds back to `at`, and the
  walk stops advancing: the dash branch pushes zero-length segments until the OOM killer takes the
  process (reproduced: SIGKILL 3.5 s into the regression test on the OLD walk), the gap branch
  spins with the UI thread frozen. Measured with an exact-f32 model over the REAL at-rest wall
  geometry (130 dotted polylines out of preview.svg): every painted ÷1.2/×1.2 ladder value hangs
  130/130 (0.8333…, 0.6944…, 0.5787…, 0.4823…, 0.4019… down; 0.36, 0.432, 0.5184, 0.6221, 0.7465,
  0.8958 up); only z = 1.0 survives because 1/4/5 are exact binary there — the default view is
  clean and the FIRST zoom click kills the shell. Fixed in `crates/sparq-app/src/ui/displaylist_egui.rs`:
  the walk computes in f64 (sub-femtopixel drift over any line the wall can draw) plus a totality
  guard (a step that fails to advance ends the run; what it drops is sub-ulp ink). Two regression
  tests pin the exact ladder values + a multi-segment carried-phase run; proven failable per the
  house rule. Cell measured: `sparq-app --features ui` 45/46 (the one red is
  `ui::live::…level_follows` — environment-sensitive, fails identically on the PRISTINE tree here;
  it is a sandbox fact, not this change's).
* **#98 — stage 4's moved hash (`5acfc1e6…` ≠ the harness pin `bdf59cdb…`): the checked-in
  component was STALE.** `instruments/observatory/observatory.wasm` (`3f775856…`, 206 081 B) predates
  INC4's `cell_ground` draw-order fix — the core moved, the harness golden re-pinned, and nobody
  rebuilt the component; the device faithfully rendered the OLD logic. Proven three ways: the
  rebuilt component (documented recipe, rustc 1.99.0 + the pinned wasm-tools 1.261.0, same paths)
  exports `observatory_core::render::cell_ground` and the checked-in one does not; the checked-in
  `preview.svg` (the device's stage-6 sync-back) carries all 16 cell grounds at 78–100 % of file —
  ground painted OVER the bodies, the exact bug INC4's fix moved under — while the current core's
  harness output puts the first ground at 0.1 %; and the host-side pipeline is parity-proven by
  reading (both IR writers field-for-field identical, serde_json without `preserve_order` on both
  sides, fixtures all-LIVE at replay-now so `window.snapshot(now)` == `iter()`, frame/params/LOD
  identical). Remedy: repackaged with the rebuilt component **`4b29a5ae…`, 206 316 B** (the
  packager dropped the stale `preview.svg` — stage 6 regenerates file 5 of 5 on the next device
  validate, from the FRESH component this time). The cross-boundary pin does NOT move: bdf59cdb is
  the current core, and the harness golden is green in the sandbox (6/6).
* **#99 — stage 4's moved fuel (`12 707 802 vs 12 401 514`): the stage compared the FIRST draw
  against the second — a cold heap against a warm one.** wasmtime 49 meters `memory.grow` per page
  (`translate_memory_grow` charges `operator_cost.variable().memory_grow_per_page` — read in the
  pinned source), and the guest's first draw grows linear memory for the wall's items/strings;
  every later draw reuses the warm free-list. Both numbers are fully deterministic — TWO
  independent loads each burned EXACTLY 12 707 802 on their first draw (the smoke stage's and the
  golden's) — so the failure words ("find the unseeded randomness or the clock read") named the
  wrong class; there was no randomness anywhere. Fixed in `runtime/stages.rs::golden_stage`: one
  warm-up draw (discarded) before the measured pair, so "×2 bit-exact" compares identical steady
  states; stage 5 KEEPS the cold draw on purpose (the first production frame pays it, so it is the
  worst case the declared budget must cover). The failure words grew the fuel-only sentence.
* **The stage-5 FAIL was the measurement the r4 session predicted** ("its real full-wall draw cost
  was never measured anywhere"): draw 12 707 802 cold / 12 401 514 warm vs the declared 2 000 000;
  process blocks 9 260; memory 1.2 MB / 3.2 MB of 64 MB. **Operator ruling 2026-10-07:
  `capabilities.max_fuel` re-baselined 2 000 000 → 32 000 000** — the measured cold worst case
  with ~2.5× headroom for a wall whose sixteen cells are all live; the ceilings moved up WITH the
  measurement, recorded here and in `gen_manifest.py`'s comment + the package README. Manifest
  regenerated (`gen_manifest.py --check` green), package reassembled (4 files until the device's
  stage 6 writes the 5th). The process-block watchdog loosens with the number (9 260 measured vs
  a 32 M budget); the epoch deadline and the frame-skip words still bound wall time — the §3 row.

**Sandbox facts this session (the environment verdict, extended):** no `curl` (rustup-init was
fetched with python `urllib`), no `xz` (zig's tarball unpacked with python `lzma`), apt hangs
(no reachable repos) — the C linker is **zig cc 0.17.0** (`-target x86_64-linux-gnu` behind a
two-line shim, `linker =` in `$CARGO_HOME/config.toml`); toolchain kept OUTSIDE the snapshot
(`/tmp/tools/{rustup,cargo}` — `.rustup` under `/home/user` would eat the 128 MB cap). The 1 GB
wall re-confirmed from a third direction: `sparq-host-wasm --features instrument-host` OOM-kills
in `cranelift-assembler-x64` — the runtime stages stay device-first-compile; the stages.rs patch
here is desk-checked against the call patterns around it. Measured in the sandbox: observatory
workspace goldens 6/6 (the pin holds), `sparq-app --features ui` dash cell green, the rebuilt
component's WIT byte-identical to the old one's (`wasm-tools component wit` diff empty).

**Delivered as r7 (this session):** `sparq-update-2026-10-07-wo020-inc5r7.zip` (workspace root,
one level above the repo; **488 entries**, FULL-TREE — overlays any tree at or after the r6
pack; the standing exclusions; **the stamp does not ride and did not move from the sandbox** —
the device re-stamps `--sync sparq-wo020-inc5r7-2026-10-07`). sha256 + size:
`handoff/sha256sums-wo020-inc5r7.txt` + the delivery message. Durable history:
`handoff/sparq-wo020-inc5r7.bundle` (`7ed2e08..HEAD` at the documents seal) + its `.sha256`
sidecar; the artefacts commit (sha256sums + the bundle itself) lands after the bundle, as
always. Packer: `handoff/make_pack_wo020.py`, **RECONSTRUCTED this session from the
2026-10-06 delivery table's description** (the original rode the sealed tree and was lost in
the re-publication; the header declares the reconstruction, the #96 precedent) — its gates ran:
clean tree at pack time, 488 == the documents' count, the MUST-NOT-MOVE surfaces (WIT,
`modules/`, the stamp, the fixtures, the goldens) unmoved vs `7ed2e08`, entry-by-entry zip↔tree
hashes green, the component `4b29a5ae…` asserted inside the zip. **One hand-step rides in every
document:** the pack ships WITHOUT `instruments/observatory/preview.svg` on purpose (the stale
sync-back, #98) and an overlay zip cannot delete — the device removes it by hand at apply;
stage 6 rewrites it from the fresh component. Expected on the re-run: [D] stage 4 words
`golden render ×2 bit-exact: bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa`,
component `4b29a5ae…`, equal warm fuels; stage 5 inside the 32 M class; GATE: COMPLETE + PASS;
the shell survives the zoom ladder with the wall loaded (#97's fix is host-side — it rides the
same pack).

### Device session 8 — 2026-10-07 (r7 on SATURN: **[D] GATE: PASS — the whole chain**; #97 confirmed by the operator's eyes; the gates.bat row was CANCELLED, not failed)

The operator applied r7 (hand-delete of the stale `preview.svg` included), re-stamped
(`sync_check: OK — sync sparq-wo020-inc5r7-2026-10-07, 175 of 175 stamped files byte-for-byte,
src 103f/2948000B`), built ([A] release, `instrument-host,streams,streams-net` + the UI/HAL
stack) and ran test007. The acceptance lines, RECORDED from `logs\test007.log`, not invented:

* **[B] the live probe from SATURN: 20/23 HTTP 200 · 2 other status · 0 transport failures ·
  1 refused pre-fetch in words.** The two others are `space.neo` + `space.epic` at **429 on the
  shared DEMO_KEY, retried once** (the recorder's policy) — a rate-limit fact of the probe hour,
  not a defect; `SPARQ_NASA_API_KEY` clears it (README §keys). FIRMS stays KEY NEEDED in words
  (D14). §3.3's asterisks are device facts now, twice over.
* **[D=J] `sparq mod validate instruments\observatory\` — GATE: PASS, the whole chain; "this
  package loads at launch."** All seven stages green on the r7 component `4b29a5ae…`:
  smoke 9/9 (process 9 260 fuel/block; save-state 109 B bit-identical ×2; first draw
  12 680 530 fuel — the COLD number, as expected post-#99); **stage 4 `golden render ×2
  bit-exact: bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa` — the harness's
  pin, reproduced over the WASM boundary (the cross-boundary parity the D13 interchange
  promises), 361 items at 2176×1120 Full, draw fuel 12 390 844 — the two runs EQUAL (the
  warm-up fixed #99; prepare burned 8 502 640)**; stage 5 inside the declared class (worst
  block 9 260 of 32 000 000; one full draw 12 680 530; memory 1 245 184 B / 3 211 264 B of
  64 MB — the re-baseline ruled this morning covers the measurement with ~2.5×); stage 6 the
  three breakpoints inside the medium ceilings (v22343/i497/h16264 · v11823/i437/h16200 ·
  v927/i403/h16200), `preview.svg` regenerated from the FRESH component (1 554 915 B — the same
  size as the stale one: the cell_ground fix reordered items, it did not add or remove any),
  at-rest re-published; stage 7 unsigned → BADGED, disabled by default in Perform.
* **#97 DEVICE-CONFIRMED by the operator's own words: "the zoom is working without OOM."** The
  zoom ladder with the Observatory loaded — the report that opened session 7 — is clean.
* **[H] headless 120: frame logic min 328 µs · med 377 µs · p99 622 µs; layout audit PASS
  (0 violations, 49 dense badges).**
* **[K] gates.bat: CANCELLED, not failed — recorded so nobody reads the row wrong.** rustfmt ✓,
  clippy default ✓ (1.39 s), clippy audio+hal ✓ (7.39 s); the `tests` row printed `[FAIL]` with
  EVERY rustc/link.exe exit code `0xc000013a` = STATUS_CONTROL_C_EXIT — a Ctrl+C during the
  from-scratch debug rebuild (gates.bat clears the cargo fingerprints on purpose so every row
  measures THIS tree), zero actual test failures; the instrument-host clippy row was then
  cancelled mid wasmtime debug compile (`^C` ends the log). The row stands OUTSTANDING, not
  red: re-run `scripts\gates.bat` and let it finish — the debug rebuild + wasmtime's debug
  compile is long (expect longer than [A]'s release build; it is not hung). The 12 runtime
  tests live in that cell; [D] already proved the same golden_stage code path end to end, but
  the gates row is the formal line and wants its own green.
* **[G] airplane mode and [I] the token re-theme: operator eyes, not yet sent** (the run sheet's
  screenshot list stands).
* **Found-by-running-it, landed in-tree (rides the NEXT pack; the deployed r7 zip is unchanged,
  the session-1 precedent):** test007.bat's [C] echo still quoted the INC3 component ("206 081
  B, sha pinned in WO020-STATE.md's INC3 record") — the pack now ships `4b29a5ae…`/206 316 B,
  pinned in THIS session-7 record; the word is corrected. [C]'s assertion (existence) was and
  stays correct. Run-sheet §2 grows the "do not cancel [K]" words with this session's evidence.

**Status after session 8:** WO-020's device acceptance is green where it ran — the hand-in gate
is COMPLETE + PASS on the device for the first time, the instrument is launch-loadable, and the
three session-7 defects are device-confirmed fixed. Outstanding: the [K] gates row (a re-run,
not a fix), the [G]/[I] eyes, and INC5b (the launch wiring: discovery → registration → PLAY
#58 → the shell's live-provider swap — until then the wall shows at rest, which is what [E/F]
eyed). **INC5b is now SUPERSEDED: the operator's window report of the same evening commissioned
`WO020-INC6-PLAN.md` ("the live instrument") — the launch wiring is its slice S4, with the card
controls, the resizable window, the key field and the per-panel poll rates the report asked
for; the four rulings (O-1…O-4) are recorded in the plan's §0.**

### Delivery — 2026-10-07 (r8: the session-8 records + the INC6 commission)

Operator instruction: **"prep handoff, sync zip and new session prompt."** The records delivery
in the r6 house shape — RECORDS, not behaviour (nothing compiled moves except test007.bat's echo
words; device application is optional and the r7 stamp stays valid for an r7 tree):

| artefact | what |
|---|---|
| `sparq-update-2026-10-07-wo020-inc5r8.zip` (workspace root, one level above the repo) | **FULL-TREE pack, 489 entries** — overlays any tree at or after r7; the standing exclusions; **the stamp does not ride and did not move from the sandbox** (the device, IF it applies, re-stamps `--sync sparq-wo020-inc5r8-2026-10-07`). sha256 + size: `handoff/sha256sums-wo020-inc5r8.txt` + the delivery message. Packer gates ran: clean tree at the seal, 489 == the documents' count, MUST-NOT-MOVE unmoved vs `7ed2e08`, entry-by-entry zip↔tree hashes, the component `4b29a5ae…` asserted inside. |
| `handoff/sparq-wo020-inc5r8.bundle` (+ `.sha256`) | `7ed2e08..HEAD` at the documents seal, SELF-CONTAINED from the published main — one fetch restores sessions 7+8 + the r8 records into any clone. Supersedes the r7 and session-8 bundles (kept as the increment record). |
| `WO020-INC6-PLAN.md` (ships inside the pack) | **the commission**: the operator's window report quoted verbatim, the four rulings (O-1 resizable half-size card · O-2 STREAMS tab + card rate fields · O-3 the 10 s floor · O-4 the user-data key file), D15–D20, slices S1–S5 with acceptance, the honest states. Next action anywhere: **S1**. |
| `RESUME.md` (ships inside the pack) | **the new-session prompt, REWRITTEN** — the 2026-09-22 resume was four work orders stale (its own fold-it-in rule); the rewrite is sized to load whole and carries the state, the outstanding list, the read-by-range table, the sandbox recipe pointers, the artefacts and the house rules that bite. The old text lives in git history. |
| `SYNC.md` + the run sheet | new top entries (r7 demoted to Previous per the house pattern); the run sheet grows the r8 revision note, the r8 sync-bundle/namelist paragraphs and the r8 re-stamp word; test007.bat's [0] remedy + [C] echo now name the r8 stamp and the shipped component. |

**Branch state at the r8 seal:** `main` in the sandbox clone off the published `7ed2e08`:
session-7 fixes `4af7f3d` → r7 documents+packer `8608a31` → r7 artefacts `4ab6827` → session-8
records `4c017e3`+`e0642a0` → session-8 bundle `3289093` → the INC6 plan `1bb9656` + the LATER
supersession `3db1ef5` → this delivery's documents seal; working tree clean at the pack; nothing
pushed (no credentials — the bundles + the pack are the delivery, ROUND8's discipline).

### Slice 1 — the stream plane's live half (`streams-net` LANDED) — commit `83b2091`, MEASURED

`sparq-streams::fetch` (the endpoint fill mirroring `streams_record.py` exactly — `{d-N}`/`{D-N}`/
`{lat}`/`{lon}`/`{st}`/`{CC}`/`{KEY}`, one-retry-on-429/5xx, key redaction: a resolved env key
exists only inside `Request::url`), `sparq-streams::broker` (cadence floors count ATTEMPTS;
**replace-vs-accumulate is registry DATA** — new `accumulate` column, exactly the three SWPC
`summary/*` rows append monotonically with dedup, INC3 finding #7's remedy; last-good cache seeds
relaunches; failures keep windows and leave words), `HttpTransport` (ureq 3.4.2 + rustls/ring +
webpki-roots — compiles AND runs in the 1 GB box), `BrokerProvider` (the live `StreamProvider`
behind the same trait, `SharedBroker` = the `HealthMirror` idiom), the CLI's real `probe
[--live]`/`fetch`/`tail` verbs, and **`record-fixtures` stays a refusal naming its owner**
(tools/streams_record.py — one owner of the fixture format; a ruling to record, reversible).
The plane's only two impure helpers (`now_unix`, `sleep_millis`) are the ADR-011 §5 wall-clock
edge, allow-with-reason per defect #66. Python recorder gained the redaction fix (a latent INC1
leak: `_meta.json` would carry a real `SPARQ_FIRMS_KEY` on a keyed device).
**Measured:** root 1033/0/1 unchanged; streams 96/0 both cells; host-wasm+streams 28/0; app
34/0; observatory 124/0 (registry drift green over the new column); every python gate; zero-dep
promise verified by `cargo tree`; **live smoke green** (real TLS fetch of swpc.kp, 200, normalized);
**live probe from the sandbox: 22/23 HTTP 200**, FIRMS KEY NEEDED in words, `wx.alerts` resolved
here this time, `geo.wildfires` still byte-identical to `geo.eonet` (INC1 drift #2 re-confirmed).

### Slice 2 — the instrument runtime (`instrument-host`) — commit `2fdb0f9`, WRITTEN, device-first-compile

`sparq-host-wasm::runtime` = instrument-host.md §2–§4 + §7 as code: the four doors (tokens serves
the checked-in bundle verbatim; host.now/random-seed — the KERNEL's `derive_seed`, one copy — /log
with RT-violation capture; assets scoped to the manifest's declared hashes, v1 answers `not-found`
honestly; sources = the D4 resolution over `StreamProvider`, serving `window.snapshot(now)` — the
flag-stamping that makes the guest's clockless status derivation equal the broker's, which is the
golden's cross-boundary parity rule), the §2 load order (compile → identity cross-check →
prepare-before-anything on BOTH instances), the memory limiter + fuel→`Overrun` mapping (§3), the
**wasmtime `cache` feature added to the pin** (the §2.1 compile cache with NO unsafe in sparq
code — `Component::deserialize` is unsafe and the allowlist forbids unsafe in Tier-2 host code;
the pin comment records the ruling), `WasmInstrument` (the `Module` face — the registry grew
`register_instrument`/`SharedFactory` for state-capturing factories, tier-blind rules unchanged),
and the gate's runtime stages: 3 smoke (the smoke.mjs mirror), 4 golden (draw ×2 bit-exact, fuel
recorded), 5 measured (fuel + memory high-water vs the declared class), 6 visual (LOD ladder vs
the gpu ceilings, the 500-primitive Minimal rule, the unknown-token scan, `preview.svg` through
the SHELL's painter, the at-rest publish — the LATER door). `validate::run_gated(dir, ctx)` runs
the FULL chain when the caller names the world; `sparq mod validate` does so under the app's
`instrument-host` feature. The noop fixture was REBUILT in this sandbox (SDK 1.1.0, rustc 1.99.0,
wasm-tools 1.261.0, unknown-unknown recipe): **53 747 B, sha `df14d18f781ea477596db552e60bdb9df06344bc673593c38148e28a3d546c65`**,
pinned in `tests/instrument_runtime.rs` with the §7 plan as 12 tests (incl. the cross-boundary
anchor: the Observatory wasm path must hash to the harness's `bdf59cdb…`). Static gates measured
green on the fixture (stages 1/2/7 PASS). `scripts/test007.bat` is the §15.2 run-sheet A–L;
`gates.bat`/`build.bat`/`ci.yml` carry the feature-on cells.

### What is NOT done (honest, in words)

1. **INC5b — the launch wiring:** discovery→`register_instrument` at launch, PLAY #58's
   discharge (the executor adopting `WasmInstrument`), and the shell's live-provider swap
   (BrokerProvider + driver thread in `ui/live.rs`). The registry door and the Module face exist
   and are tested; the app-side wiring is the next slice. Until it lands, test007's E/F steps run
   against the at-rest wall (stage 6 publishes it) and PLAY still refuses in words.
2. **The device run itself:** every `instrument-host` compile/test, the FULL validate chain on
   the sealed package, the frame-time histogram, the network story, the re-theme demo — test007
   A–L, recorded not invented.
3. Known-small gaps, declared in code: `TimeInfo`'s tempo/bar fields ride defaults inside
   `process` until the executor publishes the musical clock (the type's docs say so); a corporate
   TLS-inspecting proxy would need ureq's proxy door (webpki-roots ride the binary); the wasm
   `process` marshalling allocates (wasmtime's lifting — bounded for port-less; the native
   zero-alloc proof is the Phase-5 arena work).

---

## INC3 — the guest: source project, five-file package, harness, component — CLOSED

The plan's D13 split, built: one pure core linked by BOTH the wasm glue and the native harness, so
the shipped logic and the proven logic are the same object. The component build attempt (risk §12.2)
SUCCEEDED in the ~1 GB sandbox, so the package ships its real wasm and `sparq mod validate` runs its
static half over the assembled directory.

| path | what |
|---|---|
| `tools/make_coastline.py` + `instruments-src/observatory/assets/coastline-110m.bin` | D12: Natural Earth 110m land (public domain, cached `tools/data/`) → Visvalingam-Whyatt (one shared area threshold, binary-searched to the point target, index tie-break — deterministic) → **128 rings / 3 500 points / 14 524 B** (budgets ≤ ~3 500 pts / ≤ 160 KB). Format documented in the tool: LE, `u32` magic/version/ring-count, per ring `u32` count + `i16` centidegree lon/lat pairs, rings implicitly closed. `--check` is the staleness gate. Embedded via `include_bytes!` (the wasm data section, guide §3 small-asset rule). |
| `instruments-src/observatory/Cargo.toml` | its OWN workspace (core/wasm/harness) on the `tools/sparq-module-guest` precedent — the wasm face needs wit-bindgen and the root's default build stays zero-dep. **`exclude = ["../../tools/sparq-module-guest"]` is load-bearing**: without it, path-dep auto-membership makes `cargo fmt --all` in this workspace REFORMAT the frozen SDK (measured, twice). House lints mirrored; release profile `strip = "debuginfo"` (component 2.9 MB → 206 KB, name section kept for device trap messages). |
| `instruments-src/observatory/gen_manifest.py` | `package/sparqmod.toml` + `core/src/streams_table.rs`, both from `streams.toml` (never hand-typed): 16 cell enums × (OFF + 23 registry ids in file order), `selected_cell`, `layout`, five bools (**float defaults — the §10 `true` gap closed**), three floats (**history in SECONDS 600…604 800 default 10 800 exp — the §10 `min` gap closed**, UNITS is closed), the §5.2 toolbar as 27 generated widgets (the sixteen STREAM `enum_select`s carry `visible_if = { param = "selected_cell", equals = <option INDEX> }`), display `wall` (min_size 2176×1120, colormap.thermal) + 16 `stream = "param:cell_NN"` sources, port-less (D5), `network = "none"`, `gpu_class = "medium"`. The streams table is the guest-side registry mirror (no fs/network in T2). `--check` gates both artefacts. |
| `observatory-core` (pure, zero deps) | `ir.rs` the display-list IR, field-for-field display.wit v1 (Lod ordered detail-ascending so `>= Reduced` reads as detail); `record.rs` the third mirror of the frozen data-record (D13 — the harness and the glue own the two mappings, both drift-tested); `layout.rs` §5.1 maths as tested functions; `params.rs` the 26-param decode (floats = raw domain values, the native precedent; enums = option index; bools 0/1; clamping, NaN-index fallback) with **const-asserts proving 26 ≤ 32**; `state.rs` the deterministic blob (`OBS1` v1, magic+version+26×f32 LE = 109 B; **params-only — the scroll phase is display-instance animation and save_state runs on the audio instance, instrument-host §4**); `coastline.rs` the asset reader (refuses bad magic/version/truncation/trailing bytes in words); `view.rs` window→view (clock-free: x-windows end at the newest record's own `t_utc`); `render/*` the seven §5.5 renderers + §5.3 chrome + §5.5 ticker + §5.7 LOD ladder; `wall.rs` draw + budget counter. |
| `observatory-wasm` | the SDK glue: `process` = the port-less no-op returning `Ok` + the exact empty negotiated shapes while mirroring `block-input.params`; `draw` = 16 × `sources.snapshot("wall","cell-NN")` → core records → `Wall::draw` → IR→WIT conversion (total, tested); `configure`/`save_state` = the params blob; KEY NEEDED derived from the served window + the stream's `key_env` (D14). 10 native tests (the export macro is wasm-gated, so the glue is host-testable). |
| the component | **built in the sandbox**: `rustup target add wasm32-unknown-unknown` + `cargo build --release --target wasm32-unknown-unknown -p observatory-wasm` + `wasm-tools component new` (v1.261.0 prebuilt x86_64-linux, GitHub release — `cargo install wasm-tools` is too heavy for 1 GB) → `wasm-tools validate` CLEAN, **206 081 B**, world imports `types/sources/audio/display`, exports `sparq:instrument/guest@1.1.0`. The `wasm32-wasip1` attempt was made first and REJECTED with evidence: wasip1 std startup imports `wasi_snapshot_preview1::environ_get`, which the host deliberately does not provide (capability by absence, WIT README decision E) — `component new` refuses it. The corrected recipe (WIT README) is unknown-unknown. |
| `observatory-harness` | `to_core` (broker windows → core records, D14 key-words), `tokens` (the checked-in bundle as resolver — unknown id skips with a diagnostic, hairline opacity read from `color.hairline.regular`), `ir_json` (the D13 interchange; BTreeMap-sorted keys + deterministic floats ⇒ stable hashes; NaN → JSON null = the no-cell sentinel), `svg` (the minimal painter — INC4's `displaylist.rs` stand-in over the SAME IR; heat grids downsampled to ≤ 8192 cells FOR THE SVG ONLY, the IR keeps full resolution). Renders the §5.4 default wall at the three breakpoints' LODs; artefacts gitignored, regenerable. 22 unit tests + `tests/golden.rs` (6): **golden IR sha256 `bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa`** (re-pinned INC4 when the interchange gained `display_w`/`display_h` — a format change, reason recorded at the pin) (hand-rolled sha256 with published vectors), streams_table↔registry field-for-field, Metrics↔bundle, manifest-defaults↔core-defaults, all 26 emitted token ids resolve. |
| `tools/observatory_package.py` + `instruments/observatory/` | the assembler copies, never invents: refuses a stale generated manifest (`gen_manifest --check`), a missing hand-authored file, or a missing component (a half-package fails stage 2 fatally). Assembled **4 files ≤ 5 cap**: `sparqmod.toml`, `observatory.wasm`, `example.sparqpatch` (§5.4: the wall beside `mod/clk→mod/seq→syn/sine→util/vca→out/main`, silent-but-live), `README.md` (keys §11.4, attribution §11.3 incl. the Open-Meteo non-commercial flag, budgets). `preview.svg` absent BY DESIGN (stage 6 generates it; `package.rs` makes absence advisory). |
| `crates/sparq-audio/tests/portless_instrument.rs` + `bridge.rs` unit tests | the D5 proof as TESTS (+9): the shipped manifest validates port-less; registration accepts it; the executor builds a port-less-ONLY graph; the wall coexists with the audio path and the set still plays; `bridge::build` with the wall in the canvas graph; `browser_catalog` lists it (instrument layer, 26 params); `node_bands`/`node_size`/full `layout::compute` never refuse a zero-port spec. Both read the CHECKED-IN package manifest (include_str!/path), so a port-growing ship fails by construction. |
| `design/tokens/layout.toml` + regenerated | additive: `[marker] radius_s/m/l = 3/5/8` — the `points-item.size` token class §5.5's "size class by magnitude" requires (a size token id with no bundle entry is an unresolvable primitive). `token_gen --check` deterministic; `token_bundle.rs` pin test green; no new `token_audit` violation. |
| `tools/streams_record.py` (hygiene, first commit of the session) | the CI `check_text_io` gate was **RED on main** (INC1 left two bare pathlib `write_text` calls + no UTF-8 stream reconfigure). Fixed with the house helper pair; gate green; `--list` unchanged. |

**INC3 gates (all MEASURED this session):** `sparq mod validate instruments/observatory/` → stage 1
**PASS**, stage 2 **PASS** (4 files, roles recognised, component present), stages 3/4/6 **REFUSED IN
WORDS** (the runtime half), stage 5 static half PASS, stage 7 unsigned→badged; **GATE: PARTIAL,
exit 1 by design** ("a partial gate is not a hand-in"). `sparq mod list` → `dat/observatory v0.1.0
… loadable` + the preview.svg advisory. Root `cargo test --workspace` **991/0/1-ignored** (+9 over
INC2). instruments-src workspace **124 lib/unit + 6 golden/drift** tests. clippy `--all-targets
-D warnings` CLEAN in BOTH workspaces. fmt CLEAN (root `--all`; instruments-src PER PACKAGE — `--all`
there adopts the SDK). Python gates: `token_gen --check`, `unsafe_audit`, `module_docs --check`,
`check_text_io`, `sync_check --self-test`, `make_coastline --check`, `gen_manifest --check` all PASS;
`token_audit` still fails the pre-existing §18 mockup SVGs (no INC3 file flagged; that gate's owner
is the mockup work). WO-005 golden hash `ba577186c988db21` UNCHANGED (printed and compared). WIT
pins, the 24 module manifests, `SYNC-STAMP.txt` UNTOUCHED. Harness SVGs reviewed in-session at the
three LODs against the display-sheet idiom (aurora oval + colourbar, coastlines, quake/EONET/GDACS
dots, ISS footprint ring + crosshair, Kp bars + storm line, phosphor-green traces, AQI gauge sweep,
WWV raw text, the scrolling ticker; Reduced drops graticules/labels, Minimal keeps renderers under
the 500-primitive rule + the ticker).

**Found by building it (recorded, not papered over):**
1. **§5.1 table erratum:** the 4×4 cell height prints 270; the plan's own gap (`space.4` = 16) gives
   `round((1120 − 3·16)/4)` = **268**. The formula is the source of truth (layout.rs test asserts
   268 with a comment; every other table value reproduces exactly).
2. **`cargo fmt --all` adopts path-deps' workspaces** — in instruments-src it reformatted the frozen
   SDK; the workspace `exclude` fixes it, and the house rule here is per-package fmt.
3. **`wasm32-wasip1` cannot componentise** under the no-WASI host (environ_get import); the contract's
   own corrected recipe (unknown-unknown) is the path.
4. **Draw-order bug caught by LOOKING at the render:** the cell chrome painted the inset ground
   AFTER the body, burying every renderer; split into `cell_ground` (under) → body → chrome.
5. **Frozen-surface frictions → LATER.md (ADR-010 review-trigger material):** `trace-item` has no box
   (per-cell series ride polylines; phosphor motion tokens unused on walls); `rect-item` has no
   colormap channel (Kp bars encode level as height + the storm line); `glyph-run` has no anchor or
   measured advance (the guest estimates 0.6 × size for right-align/truncation); one wall-wide
   `history` param vs daily feeds (POWER shows ~1 point at the 3 h default); gauge threshold bands
   belong in `streams.toml` rows.
6. **Canvas anatomy `rows = max(1)`** draws one cosmetic port row on a port-less card — not a refusal
   (D5's bar is "nothing refuses"); the D6 zero-port-band instrument card is INC4's sizing work.
7. **Summary-cadence feeds are single-record windows** in fixtures (solar-wind, bz are `summary`
   endpoints): their timeseries cells draw a dot, not a trace, until the broker accumulates live
   history (INC5). Honest first-draw behaviour, not a defect — recorded so the device run-sheet
   expects it.

**Commit/bundle discipline honoured:** four slices (`cef2385` hygiene, `48a56c2` core+generator,
`0bd7f43` glue+harness+gates, `7c2f4a7` package+D5) + the seal commit carrying this card, the
CHECKLIST entry and the LATER rows. Bundle: `handoff/sparq-wo020-inc3.bundle` (+ sha256).

---

## INC2 — the contract additions (ADR-011, stream bindings, tokens, validator) — CLOSED

The one contract addition the stream plane needs, **additive, WIT untouched** (§7.4 proved: the
`wit_snapshot` hashes are byte-identical and the WIT files show no diff). Committed on
`wo020-observatory` after the INC1 seal `c19fb83` (see `git log`; carried by
`handoff/sparq-wo020-inc2.bundle`).

| path | what |
|---|---|
| `docs/adr/011-stream-plane.md` (+ `docs/adr/README.md` index) | **ADR-011** — the stream plane: host-side broker, the `stream` source binding, the registry as shared data, env-scoped keys, the determinism firewall; alternatives rejected; the §7.4 "why no WIT amendment" proof recorded; the Phase-5 review trigger. |
| `crates/sparq-module-api/src/error.rs` | `CodeKind::StreamUnknown` = **`E-STREAM-UNKNOWN`** (the catalogue's 26th code; `ALL` 30→31, count test moved in-commit). |
| `crates/sparq-module-api/src/decode.rs` | `DISPLAY_SOURCE_KEYS` +`stream`; the source binding is now **exactly one of `port`/`stream`** (`E-CROSS-FIELD`); `check_stream_binding` shape-checks a bare id vs `param:<id>`; `check_widget_visible_ifs` pins `visible_if` to `{param, equals}` (plan D7). +2 decode tests. |
| `crates/sparq-host-wasm/src/validate.rs` (+ `Cargo.toml` dep on `sparq-streams`) | stage 1 resolves `stream` bindings against the checked-in registry (`E-STREAM-UNKNOWN`), the `param:` rules (exists / is enum / options ⊆ registry, `E-CROSS-FIELD`+`E-STREAM-UNKNOWN`), and `visible_if.param` existence. Registry loaded lazily; port-less instruments accepted (D5). |
| `crates/sparq-host-wasm/tests/stream_bindings.rs` | **9 tests**: the representative Observatory draft passes stages 1–2 with real bindings; each hostile variant (unknown id, `param:` missing/non-enum/option-outside-registry, `visible_if` unknown param, port+stream both) fails its rule in words; **every one of the 23 registry ids accepted as a fixed binding** (the registry↔contract drift gate). |
| `design/tokens/layout.toml` (+ regenerated `generated/**`) | `node_width_instrument_max = 2400` + the card-sizing rule (D6) as data; `token_gen.py` re-run, `--check` deterministic. |
| `docs/api/manifest-schema.md` §9 + catalogue; `docs/api/manifest-fields.toml` | the `port|stream` binding, `visible_if` semantics, `E-STREAM-UNKNOWN` + the `E-CROSS-FIELD` rules, all as data (the field-table drift tests only pin layer/gpu_class/top, untouched). |
| `MODULE-BUILD-GUIDE.md` §5 + changelog | one author-facing paragraph: stream bindings + where the registry lives (the guide's own "amend when falsified" rule). |

**INC2 gates (measured):** `cargo test --workspace` **982/0/1-ignored** (+11 over INC1's 971: 2 decode + 9 validator); `sparq-streams --features streams` still **75/0**; clippy `-D warnings` CLEAN (default workspace + host-wasm + streams cell); fmt CLEAN; `token_gen --check` CLEAN; `wit_snapshot` PASS and `docs/api/instrument-wit/wit/*` shows **no git diff** (§7.4); `api_snapshot` + the three field-table drift tests PASS; goldens and the 24 module manifests untouched. `unsafe_audit` + `module_docs --check` CLEAN. (`token_audit` still fails on the pre-existing §18 mockup SVGs — no INC2 file is flagged.)

**Two §10 draft-manifest gaps found by building it** (both are generator concerns for INC3, recorded not papered over): the draft's `unit = "min"` is **not** in the closed `UNITS` vocabulary, and its bool params use `default = true` where the decoder wants a float (`1.0`). `gen_manifest.py` (INC3) must emit valid values; the representative test manifest already does.

---

## INC1 — the stream broker's hermetic half + the registry as data — CLOSED

The stream plane's **hermetic half** — everything the ~1 GB sandbox can build and prove without a
socket or wasmtime (plan D2, §9 INC1). The default build stays **zero third-party dependencies**
(`cargo tree` verified: `sparq-streams` default pulls only `sparq-module-api` → `sparq-kernel`);
`serde_json` rides the `streams` feature, `ureq`/TLS the `streams-net` feature (the device half).

| path | what |
|---|---|
| `crates/sparq-streams/streams.toml` | **THE registry** — 23 streams (id, domain, label, endpoint template, cadence, view, schema, units, attribution, key_env/fallback, transform, config knobs) + the §3.3 dead ends recorded as comments. Single source of truth for stream ids. |
| `crates/sparq-streams/src/record.rs` | Native mirror of the frozen `data-value`/`data-flags`/`data-record` vocabulary (`types.wit` v1.1), field-for-field. Advisory-stamp rules documented. |
| `crates/sparq-streams/src/schema.rs` | The five `observatory/*@1` channel schemas as typed builders + the shared severity/visibility tier vocabularies. |
| `crates/sparq-streams/src/registry.rs` | `streams.toml` via the house TOML subset parser; `KeyState` (KEY NEEDED in words, D14); the `E-STREAM-UNKNOWN` refusal message INC2 reuses. |
| `crates/sparq-streams/src/window.rs` | Bounded rolling windows (timeseries 2048 / events 256 / grid 2 / text 64) + the stale(3×)/offline(10×) policy. **Clock-free** — `now` is injected (D11 firewall). |
| `crates/sparq-streams/src/time.rs` | Dependency-free timestamp parsing (ISO-8601 family ± offset/frac, compact `YYYYMMDD`/`YYYYMMDDHHMMSS`, SWPC `:Issued:`), no chrono, no `SystemTime`. |
| `crates/sparq-streams/src/normalize.rs` *(streams)* | All 23 feeds → frozen `data-record`s, written against the **recorded** payloads. Severity tiers, ISS km/h→km/s, aurora 2° regrid (16 200 cells), POWER fill-value skip, NWS null-geometry tolerance. |
| `crates/sparq-streams/src/cache.rs` *(streams)* | Disk cache (last-good + metadata), env-resolved user-data dir; `File`/`read_to_string` only (honours the `clippy.toml` fs ban). |
| `crates/sparq-streams/src/replay.rs` *(streams)* | Fixtures → windows (the hermetic door for goldens/tests/INC4's `ReplayProvider`). |
| `crates/sparq-streams/tests/normalizers.rs` | 21 tests: 23 per-stream normalizers against real fixtures + the registry↔normalizer drift gate + determinism (same fixture twice, bit-identical). |
| `tools/streams_record.py` | urllib recorder; reads `streams.toml` (tomllib), fills endpoint templates, writes raw payloads + `_meta.json` + `PROBE-LOG-<date>.md`. |
| `reference/fixtures/observatory/` | All 23 streams recorded at HTTP 200 (FIRMS synthetic — free key unavailable), raw payloads + `_meta.json` + `PROBE-LOG-2026-10-06.md`. |
| `crates/sparq-app/src/streams.rs` | `sparq streams list|cache` (hermetic) + `probe/fetch/tail/record-fixtures` refuse in words, exit non-zero without `streams-net`. Exit codes testable via `run_code() -> u8`. |
| `Cargo.toml` (root + app) | `sparq-streams` member; `serde_json` optional workspace dep; app `streams`/`streams-net` features. |

## Gates (all MEASURED this session, not invented)

* Pristine baseline before any edit: **925 / 0 / 1-ignored** (the round-8 seal, exactly).
* `cargo test --workspace` (default, zero-dep): **971 / 0 / 1-ignored** (+46).
* `cargo test -p sparq-streams --features streams`: **75 / 0** (54 lib + 21 normalizer).
* `cargo test -p sparq-app --features streams`: green (incl. the 5 `streams::tests` CLI cases).
* `cargo clippy --workspace --all-targets -- -D warnings`: **CLEAN** (default), and CLEAN for
  `-p sparq-streams --features streams` and `-p sparq-app --features streams`.
* `cargo fmt --check`: **CLEAN**.
* Zero-dep promise: `cargo tree` shows no third-party crate in the default build of `sparq-app` or
  `sparq-streams`; `serde_json` (+ its transitive `serde_core`/`zmij`/`itoa`/`memchr`) appears only
  under `--features streams`.
* Fixture-size acceptance: every fixture is the **raw payload**, worst ratio **1.000** (≤ 1.2 ✓).
* **Untouched (as the plan requires):** `docs/api/instrument-wit/wit/*` hash pins, the goldens, the
  24 module manifests, `SYNC-STAMP.txt`.

## API drift found by recording live (the probe-first rule earning its keep, §12.6)

1. **NASA POWER 422s on dashed dates.** `power.larc.nasa.gov/.../daily/point` now rejects
   `start=2026-09-06` with `int_parsing: Input should be a valid integer`. It wants **compact
   `YYYYMMDD`**. Fix landed: the recorder + registry gained `{D-N}` (compact) placeholders
   alongside `{d-N}` (dashed); POWER's endpoint uses `{D-30}`/`{D-3}`. Re-recorded → 200, 1571 B.
2. **EONET `categories=wildfires` is ignored under `limit`.** With `limit=200`, `geo.wildfires`
   returns byte-identical content to `geo.eonet` (all open events) — EONET v3 applies `limit`
   before the category filter. Kept as its own registry row per §3.1; flagged in `streams.toml` for
   INC3/operator (drop the limit → ~5 MB, or retire the row). Not silently papered over.
3. **FIRMS needs a free key** (`SPARQ_FIRMS_KEY`); with none set it records a small **synthetic**
   CSV sample, flagged `"synthetic": true` in `_meta.json` and the probe log (D14: KEY NEEDED, never
   a silent hole). The normalizer is tested against it; a real key replaces it on the next record.

## What is NOT done (the rest of WO-020)

* **INC3** — CLOSED this session (see above).
* **INC4** — CLOSED this session (see the INC4 closeout in CHECKLIST.md). The host half in one
  sentence: `sparq-ui::displaylist` (zero-dep JSON subset → parsed IR → token-resolving `Painter` →
  SVG back end) plus the egui back end in `sparq-app` (same resolved list, dashes as segments,
  curves chorded), the at-rest store (SPARQ_ATREST / cache dir; WORDS when none), the picker
  (44 px rows, undoable select), FOCUS (menu row / header double-tap / toolbar), D6 sizing
  (2208×1288 measured; coverage audit row 97.5 % at 2560×1600), browser sections, the §5.2 status
  label from the ReplayProvider, `sparq instrument render` with the §5.8 budget vs the declared
  gpu_class.
* **INC5** — device-only: wasmtime loader, `BrokerProvider` (`streams-net` + `fetch.rs`), the full
  validate chain, live acceptance (run-sheet §15.2 / `scripts/test007.bat`).
* **INC6** — deferred/gated (sonification, SGP4 tracks, GIBS imagery).

## Resume recipe (a fresh sandbox has NO Rust and NO C linker — install both first)

```sh
# 1. Toolchain (this session installed it; it does NOT persist across sandbox snapshots).
python3 - <<'PY'   # no curl/wget in the sandbox — download rustup with python
import urllib.request,os
u='https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init'
d=urllib.request.urlopen(urllib.request.Request(u,headers={'User-Agent':'sparq'}),timeout=280).read()
open('/tmp/rustup-init','wb').write(d); os.chmod('/tmp/rustup-init',0o755)
PY
/tmp/rustup-init -y --profile minimal --default-toolchain stable --component rustfmt --component clippy --no-modify-path
# 2. A C linker (rustc needs `cc`; the sandbox has none). apt worked on 2026-10-06 and HUNG on
#    2026-10-07 (no reachable repos) — the fallback that needs no package manager is zig cc:
apt-get update -qq && apt-get install -y --no-install-recommends gcc libc6-dev   # try first
# ...else (session-7 measured; no `xz` in the box either, so python unpacks the tarball):
python3 - <<'PY'
import urllib.request, lzma, tarfile
u='https://ziglang.org/download/0.17.0/zig-x86_64-linux-0.17.0.tar.xz'
urllib.request.urlretrieve(u, '/tmp/zig.tar.xz')
with lzma.open('/tmp/zig.tar.xz') as fh, tarfile.open(fileobj=fh) as t: t.extractall('/tmp/tools')
PY
printf '#!/bin/sh\nexec /tmp/tools/zig-x86_64-linux-0.17.0/zig cc -target x86_64-linux-gnu "$@"\n' \
  > /tmp/tools/zigcc && chmod +x /tmp/tools/zigcc
printf '[target.x86_64-unknown-linux-gnu]\nlinker = "/tmp/tools/zigcc"\n' > "$CARGO_HOME/config.toml"
# Keep RUSTUP_HOME/CARGO_HOME OUTSIDE /home/user (e.g. /tmp/tools/{rustup,cargo}): the snapshot
# caps around 128 MB and a toolchain under the workspace eats the cap (session 7's lesson).
# 3. Gates (root workspace):
export PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup"
cd /home/user/sparq
cargo test --workspace                                   # 991 / 0 / 1  (INC3 seal)
cargo test -p sparq-streams --features streams           # 75 / 0
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p sparq-app --features streams && target/debug/sparq streams list
target/debug/sparq mod validate instruments/observatory/  # PARTIAL exit 1 BY DESIGN (static half)
# 4. The Observatory workspace (its own gates; fmt PER PACKAGE — --all adopts the frozen SDK):
cd instruments-src/observatory
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -p observatory-core -p observatory-wasm -p observatory-harness --check
cargo run -q -p observatory-harness -- --report           # §5.8 budget vs medium ceilings
python3 ../../tools/make_coastline.py --check && python3 gen_manifest.py --check
# 5. The component (sandbox-proven recipe; wasm-tools PREBUILT — cargo install is too heavy for 1 GB):
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown -p observatory-wasm
python3 tools/fetch_wasm_tools.py   # prints the exec path (pins wasm-tools 1.261.0)
/tmp/wasm-tools component new target/wasm32-unknown-unknown/release/observatory_wasm.wasm -o /tmp/observatory.wasm
cd /home/user/sparq && python3 tools/observatory_package.py --component /tmp/observatory.wasm
# 6. Re-record fixtures only if the network is up and drift is suspected (they are checked in):
python3 tools/streams_record.py --all --out reference/fixtures/observatory/
```

## House notes

* **Incident (this session, no trace left in the tree):** a broken shell heredoc in the session's own
  tooling executed the resume recipe's re-record line mid-edit, re-recording all 23 fixtures live
  (~13:51 UTC). Caught by `git status` before any commit and by the golden-IR pin afterwards; the
  fixtures, `_meta.json` and the probe log were reverted to HEAD (`git checkout -- reference/fixtures`),
  the golden re-verified green. Lesson recorded for future sessions: the re-record step is DESTRUCTIVE
  to the goldens — run it only deliberately — and check `git status` after any command accident.

* `token_audit.py` FAILS on the pre-existing §18 `design/mockups/observatory-*.svg` +
  `display-sheet.svg` (resolved hex in generated SVGs). This predates INC1 — the crate adds no
  appearance values — and is out of INC1's scope; flagged for whoever owns the mockup gate.
* Commit early/often honoured: one increment commit (`7da006a`) + a seal commit (this state card,
  the CHECKLIST entry, the bundle). Bundle: `handoff/sparq-wo020-inc1.bundle` (+ sha256).
* No operator rulings were needed beyond the two that scoped this session (INC1-only, install+verify).
