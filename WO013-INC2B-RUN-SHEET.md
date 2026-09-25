# WO-013 increment 2b — SATURN run sheet (test005, second attempt)

No product code moved in this package. It fixes the delivery path, and it adds the check that tells
us — in words, in the log — which of two indistinguishable causes broke the first `test005` run.
Scratch companion to the build log in `PHASE0-WORKORDERS.md` (search **increment 2b**, defect #78).

---

## 0. What went wrong, exactly

Your log's six errors are one shape:

```
error[E0599]: no method named `resolve_master` found for reference `&CanvasState`
error[E0609]: no field `master` on type `CanvasState`
  = note: available fields are: `camera`, `selection`, `interaction`, `menu`, `history`
```

That note is the tell. Five fields, no `master` — that is **increment 1's** `CanvasState`.
Increment 2's has six. And all six error line numbers (`canvas_ui.rs:46`, `headless.rs:549/590/601`,
`shell_ui.rs:248/483`) match the increment-2 files byte for byte, so your `sparq-app` sources **were**
the new ones. The build also prints only `Compiling sparq-app` — cargo never rebuilt `sparq-ui`.

So: new app crate, compiled against the **previous increment's compiled `sparq-ui`**. Two causes
produce exactly that, and a build log cannot tell them apart:

* **(a)** `crates\sparq-ui\src\canvas\interact.rs` never landed. This tree has form: an Explorer
  extraction once produced a stray `decode (2).rs` *instead of overwriting* (defect #71).
* **(b)** it landed, and cargo did not look at it. Extracting a zip restores the **archive's**
  timestamps, so a brand-new source file can be *older* than the cached build; cargo answers `Fresh`
  and serves the old compiled crate. That is defect #41's mechanism on the side of the failure the
  stamp guard cannot see — that guard inspects the exe **after a successful build**, and this fails
  the compile.

Reproduced and measured in the sandbox, so (b) is not a theory:

```
build sparq-ui from inc1's interact.rs .................. 1.28 s   rlib written
inc2's interact.rs in place, mtime = the archive's ......
cargo build --release -p sparq-ui -v ...  "Fresh sparq-ui", 0.06 s
   that rlib contains `resolve_master`  0 times
delete target\release\.fingerprint\sparq-*, rebuild ..... 4.20 s
   that rlib contains `resolve_master`  6 times
```

Both causes are fixed here: (a) by re-sending the file and hashing the whole tree **before** cargo
runs, and (b) by deleting the five first-party fingerprints whenever the tree's *content* differs
from what the last good build recorded — content, not timestamps, so a zip-restored mtime cannot
hide a change. An unchanged tree still builds incrementally, in seconds. Step **[00b]** in the new
`test005` says which cause you had.

## 1. Apply

Copy `sync-wo013-inc2b.zip` to SATURN and extract it **at the repo root**
(`Q:\morphosis\code\sparq`), choosing *Replace the files in the destination*. Do not `git clone`
onto the stage machine (defect #71).

```
EXTRACT-AT-REPO-ROOT.txt                  read me first
SYNC-STAMP.txt                            NEW the content manifest: sha256 + size, 120 files
crates/sparq-ui/src/canvas/interact.rs    RE-SENT, byte-identical to inc2 - cause (a) insurance
scripts/build.bat                         sync check before cargo + a content-gated fingerprint purge
scripts/synccheck.bat                     NEW the check on its own, seconds, into logs\synccheck.log
scripts/test005.bat                       NEW step [00b] + a summary line for it
tools/sync_check.py                       NEW the checker; --self-test proves all 11 cases
tools/README.md                           one row for the new tool
.github/workflows/ci.yml                  one step: that self-test now runs in CI
WINDOWS.md                                the two scripts + a troubleshooting section for this error
PHASE0-WORKORDERS.md                      increment 2b build-log entry, defect #78
WO013-INC2B-RUN-SHEET.md                  this file
```

Nothing under the five `src` roots changed, so **the build stamp does not move**: it is still
`src 80f/1348971B`. If your `sparq version` says anything else after this build, that is a find.

## 2. Ten seconds, before anything else

```bat
scripts\synccheck.bat
```

Expected, verbatim apart from the file count:

```
 sparq synccheck - tree vs SYNC-STAMP.txt

sync_check: OK - sync wo013-inc2b (sent 2026-09-25)
  120 of 120 stamped files match byte-for-byte
  src fingerprint 80f/1348971B - matches the stamp
  no extras, no line-ending drift, nothing soft differing

 verdict: the tree matches the stamp. Safe to build.
```

If it prints **FAIL**, the lines above it name the files and say whether each is `MISSING`, `SIZE`
or `HASH` — re-extract and run it again. If it prints **CANNOT RUN**, `python`/`py` is not on PATH;
that is not a pass, and `build.bat` will say so and carry on.

## 3. Then the acceptance run

```bat
scripts\test005.bat
```

Answer the listen prompt at step [05] (`y` / `n` / Enter to skip), then send back **one file**:
`Q:\morphosis\code\sparq\test005.log`.

What each step should say:

| Step | Expect |
|---|---|
| **[00b]** | the same four lines §2 shows — `sync_check: OK - sync wo013-inc2b (sent 2026-09-25)` / `120 of 120 stamped files match byte-for-byte` / `src fingerprint 80f/1348971B - matches the stamp` / `no extras, no line-ending drift, nothing soft differing` — and the summary reads `[00b] tree matches SYNC-STAMP.txt : PASS rc=0` |
| **[01]** | `sync_check: OK - 120 files match sync wo013-inc2b, src 80f/1348971B` (build.bat's own preflight, one line), then `note: tree content changed since the last good build - the five first-party fingerprints are cleared…` — this run has no marker yet, so it always says that — then **five** first-party crates compile: `sparq-kernel`, `sparq-audio`, `sparq-module-api`, `sparq-ui`, `sparq-app`. Release is `opt-level 3`, `codegen-units 1`, thin LTO, so a few minutes; that is not the wgpu-from-scratch cost, dependencies stay cached. Then `stamp guard: sources 80f/1348971B - binary 80f/1348971B`. On the *next* build with nothing changed it says `tree content unchanged since the last good build - building incrementally` and is quick again. **If you see only `Compiling sparq-app` after a sync, the purge did not happen — that is a new find, say so.** |
| **[02]** | ``scanning `modules` — 3 manifest(s) found`` · `loaded 3:` `sparq/ana/rms`, `sparq/syn/sine`, `sparq/util/gain` · rc 0 |
| **[03]** | `rendered 3750 blocks · 240000 frames · hash d46736fd9a1c48a1` · `exec: PASS` · `exec.wav` 1 920 046 bytes |
| **[04]** | `ui audit: PASS (0 failure(s))` — 16 smokes, including `empty long-press → RENDER WAV: the bridge renders the patch and writes canvas-render.wav` and `node long-press → SET MASTER makes that node the render master` · `canvas-render.wav` 1 920 046 bytes |
| **summary** | `steps failing on rc: 0` |

The two SHA-256 lines are yours to keep; for reference the sandbox measured
`aa546a79ab2f10ea0f7558211ac59553deeda90e999515785555b5c7339e6485` for `exec.wav` (three renders,
byte-identical) and `d7ad294ea0b5e6e960f8dbc7f231dcbd84edec327e6352e80dc2f6ed786ec0b7` for
`canvas-render.wav`. Those are a Linux debug build, so treat a match as a bonus and a few-ULP
difference as unremarkable — the numbers that **must** agree are the frame count (240 000) and the
file size (1 920 046 bytes).

## 4. If it still fails

Send `test005.log`. Then, in this order:

1. `[00b]` FAIL → the lines name the files. Re-extract at the repo root, replacing, and re-run
   `scripts\synccheck.bat` until it says OK.
2. `[00b]` OK and `[01]` fails with `no method named` / `no field` again → the cache outlived the
   purge. Run these and send both logs:
   ```bat
   rd /s /q target\release
   scripts\build.bat
   ```
   That would mean a **dependency** crate went stale, not a first-party one, and the purge list in
   `build.bat` needs widening — worth knowing, and the reason this branch is written down here.
3. `[01]` OK but `[04]` fails → that is a real product find: the audit drives the shell by synthetic
   touch, so a failure there is about the canvas, not the sync. Quote the failing `[PASS]`/`[FAIL]`
   lines.

## 5. Still waiting on the studio session

Unchanged by this package: `test004` (the 2 h zero-xrun soak at 96 kHz/64 **exclusive**, defect #77's
integer rungs), real fingers on the canvas, and the DPI matrix — none of it claimable over RDP.
