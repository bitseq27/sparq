# Running sparq on Windows

This is Phase A: **get sound out of your machine**, with enough diagnostics that if it doesn't work, one log file tells me why.

---

## Every script keeps a log

Every script writes its full output to `logs\<name>.log` and prints the path before exiting.
When you double-click one, the window stays open until you press a key; when you run it from an
existing terminal it returns immediately, so it is safe to use in CI or in a loop. If a window ever
closes too fast, the answer is in `logs\`.

---

## The three commands

Open a terminal (`Win+R` → `cmd`) in the folder that contains this file, then:

```bat
scripts\setup.bat       REM one time: installs Rust + the MSVC C++ Build Tools
scripts\devices.bat     REM shows every audio device on every backend
scripts\run.bat         REM makes a sound
```

**Turn your monitors or headphones down before `run.bat`.** It plays a 220 Hz tone at −12 dBFS immediately. It stops when you press `q`.

That's the whole happy path. Everything below is for when it isn't happy.

---

## What each script does

| Script | Purpose |
|---|---|
| `setup.bat` | Installs Rust (stable, MSVC toolchain) if missing; installs Visual Studio C++ Build Tools if the linker is missing; fetches crates; then runs `verify.bat`. Idempotent — re-run it any time. |
| `verify.bat` | Proves the toolchain works: checks `rustc`/`cargo`, **actually links a trivial program** (the step that fails on fresh installs), builds the crates, and runs the offline audio gates. |
| `build.bat` | Builds `target\release\sparq.exe` with the audio backend **and the UI shell** (`--features bootstrap-audio,hal-wasapi,ui-window`; the first build compiles wgpu — minutes, not seconds). Before cargo runs it checks the tree against `SYNC-STAMP.txt` and refuses to build a half-applied sync, then, **only when the tree's content changed since the last good build**, clears the five first-party fingerprints so a zip-restored timestamp can never make cargo serve the last increment's compiled crate (defect #78). An unchanged tree still builds incrementally, in seconds. `scripts\build.bat --skip-sync-check` builds a tree you edited on purpose. |
| `devices.bat` | `sparq play --list-devices` — every device, on WASAPI / DirectSound / MME (and ASIO if a driver is installed), with sample rates, channel counts and sample formats. |
| `run.bat` | Plays. Passes any extra arguments straight through. |
| `diag.bat` | **The one to run when there's no sound.** Writes `sparq-diag.log` + `sparq-diag.wav` and tells you how to interpret them. |
| `gates.bat` | Every check CI runs: fmt, clippy at `-D warnings` (both feature configs), all tests, both golden references, and the three Python gates. **Writes `logs\gates.log` and stays open when double-clicked**, so the result survives the window closing. |
| `soak.bat` | Long-run reliability soak: `scripts\soak.bat 2` for a pre-flight check, `scripts\soak.bat 120` for the Phase 0 gate. Without a backend argument this is offline CPU work: no sound, no sound card needed. With one it goes through the HAL and **makes sound**: `scripts\soak.bat 120 wasapi-exclusive` is the WO-006 acceptance run. |
| `hal.bat` | **WO-006 bring-up on real hardware.** Builds, then runs `devices`, `devices --caps`, `devices --conformance`, 10 s of `play` on each WASAPI backend, and a 2-minute paced soak — logging everything to `logs\hal.log`. Makes sound; monitors down. |
| `synccheck.bat` | **Seconds, no build.** Hashes every file that decides what the binary is against `SYNC-STAMP.txt` and names the ones that differ — the definitive answer to *did the sync land?* Exit 0 matches, 1 differs, 2 could not run (which is not a pass). `--json` for machine output. Writes `logs\synccheck.log`. |
| `ui.bat` | **WO-012 shell prototype.** Opens the window (rail / canvas / inspector / dock, token-styled, touch+mouse+pen through one gesture recogniser). `scripts\ui.bat --audit` runs the headless layout gate instead — no window, no GPU needed. Esc closes. |

### `run.bat` arguments

```bat
scripts\run.bat                              play until you press q
scripts\run.bat --seconds 10                 play 10 s, print telemetry every 2 s
scripts\run.bat --device 2                   use output device index 2 (see devices.bat)
scripts\run.bat --backend wasapi             force a backend
scripts\run.bat --rate 96000 --block 128     higher sample rate, larger block
scripts\run.bat --freq 110 --gain -20        quieter, lower
scripts\run.bat --ch 8                       request 8 channels (if the device allows)
scripts\run.bat --write-wav out.wav          also capture what sparq produced
```

Keys while playing: `]` `[` gain ±1 dB · `f` `F` frequency ×2 ÷2 · `m` mute · `q` quit.
(With `--seconds`, key input is disabled and you get a status line instead.)

---

## The real HAL (WO-006): `sparq devices`, and `--backend` on `play`/`soak`

Everything above is the **bootstrap** path (ADR-008): cpal, shared mode, a mutex in the callback,
no MMCSS. It is disposable and it is the path `run.bat` still uses, because a proven path is never
rerouted by an unproven one. The real HAL is built and sits beside it:

```bat
scripts\hal.bat                                      the whole bring-up, logged
sparq devices                                        every backend, every device
sparq devices --caps                                 probed capabilities (rates, channels, exclusive, periods)
sparq devices --conformance                          the behavioural promises, measured
sparq play --backend wasapi-exclusive --seconds 10   the stage path: no mixer, lowest latency
sparq play --backend wasapi-shared   --seconds 10    the compatible path
sparq play --backend null            --seconds 10    the virtual device: full HAL, no hardware
scripts\soak.bat 120 wasapi-exclusive                the 2 h zero-xrun acceptance run
```

`--backend` values that route to the HAL: `null`, `wasapi-exclusive`, `wasapi-shared` (aliases
`wasapi-x`, `wasapi-s`). Any other value — including a bare `wasapi` — still routes to the
bootstrap. `asio` is refused with a message naming WO-006 increment 2 rather than silently falling
back to something else.

What the HAL path adds, and how to see it:

| Claim | Where it shows up |
|---|---|
| MMCSS Pro Audio + TIME_CRITICAL actually applied | the `rt setup` line at start, and in every diagnostics dump. `NO-mmcss` means the boost did not apply — check the power plan and whether this is a remote session |
| Zero allocations on the audio path | `alloc 0` in the diagnostics. Counted, not inspected: the pump wraps every callback in the counting allocator |
| Zero xruns, and *which kind* | `xruns N (late wakes L, budget overruns M, device errors E)`. Late wakes = the system missed the device cadence. Overruns = our DSP exceeded its budget. Different cures; the split is the point |
| Callback timing under load | `callback p50 · p99 · max`, plus a power-of-two bucket histogram at the end of a soak |
| Device clock honesty | `drift ±N ppm device clock vs wall` |
| Negotiated ≠ requested | the `NEGOTIATED` line, and `sparq devices --caps`. Shared mode gets the engine's own rate and period; the report says so instead of pretending |

Over RDP the HAL behaves like the bootstrap does: you get the `Remote Audio` endpoint, its latency
numbers are network numbers, and exclusive mode will usually be refused. That is not a defect —
see the RDP section below. The acceptance numbers only mean something at the physical machine.

If `hal.bat` fails a step, it keeps going and reports the count at the end. Send
`logs\hal.log` — the whole file, not the tail.

---

## If it doesn't work

### Every render is identical no matter what arguments I pass

The usual cause is a **stale `sparq.exe`**. `demo.bat` used to build only when the executable was
missing, so after syncing new source it kept running the old binary — which accepted the arguments,
ignored the ones it did not know, and rendered the default patch every time without printing an
error.

`demo.bat` now always rebuilds, then verifies the binary understands `--list-patterns` before
rendering, and tells you which source files are missing if it does not. To check by hand:

```bat
target\release\sparq.exe version
```

That prints a build stamp — UTC build time plus a source fingerprint (`src 25f/234277B`). If `built`
is older than your last sync, or the fingerprint does not change after you sync, you are running an
old binary or the sync did not land.

The other possible cause is that the arguments never reached sparq. `logs\demo.log` records the exact
command; if the `pattern` and `mutation` lines in the output do not match what you typed, send me that
log.

### The build fails with `no method named …` / `no field …` for something that IS in the file

Run this first:

```bat
scripts\synccheck.bat
```

It answers in seconds, without building anything. Two causes look identical in a build log, and it
tells them apart:

* **The file is not the file you think it is.** `synccheck` prints
  `SIZE crates\sparq-ui\src\canvas\interact.rs — disk 43962B, stamp 51583B`, or `MISSING`. The sync
  did not fully land — Explorer extracting over an existing tree has produced `(2)` copies here
  before instead of overwriting (defect #71). Re-extract the zip **at the repo root**, choose
  *Replace the files in the destination*, and run `synccheck` again until it says OK.
* **The file is right and cargo did not look at it.** Extracting a zip restores the *archive's*
  timestamps, so a brand-new source file can be **older** than the cached build. cargo then reports
  `Fresh`, serves the previous increment's compiled crate, and every crate downstream fails with
  errors about methods that are sitting on disk (defect #78, and #41's mechanism on the side of the
  failure the stamp guard cannot see — that one checks the exe *after* a successful build). The fix
  is to delete cargo's evidence:

  ```bat
  rd /s /q target\release\.fingerprint
  scripts\build.bat
  ```

  `build.bat` now deletes the five first-party fingerprints itself before every build, so this
  should not recur — if you ever need the command above, say so in the message back, because that
  means a dependency crate went stale too.

The tell in the log, either way: only `Compiling sparq-app` appears, with no `Compiling sparq-ui`
above it, and rustc's note lists the *old* struct's fields (`available fields are: …`).

### `link.exe not found` / `linker not found` / `unable to find link.exe`

The MSVC C++ Build Tools are missing. `setup.bat` installs them, but if it couldn't (no winget, or you skipped the prompt):

1. Download **Build Tools for Visual Studio** from <https://visualstudio.microsoft.com/downloads/>.
2. In the installer, tick **Desktop development with C++**.
3. Open a **new** terminal and re-run `scripts\setup.bat`.

### `no output devices` / device list is empty

* Windows sound settings → is a playback device enabled and not disabled?
* Is the interface powered on and connected? Try a different USB port (not a hub).
* Is another program holding the device exclusively (a DAW, Discord, a browser)? Close it.
* Try `scripts\run.bat --backend directsound` — a different backend sometimes sees what WASAPI won't.

### It ran, printed a final report, and you heard nothing

This is the case `diag.bat` exists for. Run:

```bat
scripts\diag.bat
```

Then **open `sparq-diag.wav`** in any audio editor (or just play it):

| What you find | What it means |
|---|---|
| The WAV contains a clean sine | sparq is working. The problem is downstream: wrong playback device, Windows mixer muted, or interface output routing. Check the OS volume mixer for `sparq.exe` specifically. |
| The WAV is silent, and the report says `peak 0.000000` | sparq produced nothing — that's my bug. Send me `sparq-diag.log`. |
| `callbacks 0` | The device accepted the stream but never asked for audio. Usually exclusive-locked by another app or a stalled driver. Try `--block 256`, a different `--device`, or a different `--backend`. |
| `frames delivered` is much less than expected, `frame deficit` large | The callback isn't keeping up or is being starved. Send me the log; this is the interesting Windows scheduling case. |

### It works but crackles / drops out

* Increase the block: `scripts\run.bat --block 256` (or 512).
* Lower the sample rate: `--rate 48000`.
* Check the `est-underrun` counter in the final report — if it's nonzero, the callback is overrunning its budget.
* This bootstrap path is **shared mode with a mutex in the callback and no MMCSS priority**. That is a known, deliberate, temporary defect (ADR-008). Fixing it properly — WASAPI exclusive, MMCSS `Pro Audio`, working-set lock, DPC mitigation — is exactly what WO-006 is for. Some crackle at small block sizes is expected until then.

---

## Remote desktop (RDP) and audio interfaces

**Windows RDP does not pass USB audio devices into the session.** It offers a virtual endpoint
called `Remote Audio`, which renders on the *client* machine over the network. An interface plugged
into the host — a Behringer UMC204HD, say — is physically present and logically invisible.

The signature in a diagnostic log:

```
[WASAPI]
  out [0] Remote Audio
        default: 44100 Hz · 2 ch · F32
        rates 44100..44100 · ch 2..2 · fmt F32/I16/I32/U8
```

One device, name containing "Remote", locked to 44.1 kHz stereo. `sparq` now detects this pattern and
warns at startup.

And the timing tells the same story: callback gaps of 9–69 ms against a 10 ms device period, because
the endpoint delivers in bursts over the network. sparq still produced correct audio (peak measured
to within one f32 ULP of the expected value), but nothing about that path can be used to judge
latency, underruns or multichannel behaviour.

**What this means concretely:** RDP audio cannot support this project's real-time goals — sample
accuracy, <10 ms round trip, 8+ outputs, ASIO. It is fine for *hearing* that the engine works.

### Your options

| Option | Verdict |
|---|---|
| **Develop over RDP, verify by offline render** | **Best available now.** `sparq render` needs no device at all; listen to the WAV on any machine. Bit-exact and deterministic, so it is a *stronger* check than live playback for DSP work. All of Phases B, C, E and most of D need no sound card. |
| **Work at the physical machine** | Required eventually. A touch-first performance instrument has to be played where the touchscreen and the interface are. Also the only way to verify WO-006. |
| **RDP USB redirection (RemoteFX)** | Only on Windows Enterprise/Education or Server, needs Group Policy changes on both ends, and USB *audio class* redirection is unreliable. Worth one attempt; do not plan around it. |
| **A remote tool with real USB forwarding** (VirtualHere, NoMachine, Parsec) | Practical for a single interface. Adds its own latency, so still not suitable for critical listening. |
| **Headless on the host + thin client** | The plan's own answer (D-10, Phase 6): sparq runs on the machine with the interface, visuals and controls come to you over the network. Not built yet. |

### Hearing the instrument without a device

```bat
scripts\demo.bat
```

Renders four bars of the Phase B patch — kick (sine + pitch envelope + noise transient through the
SVF), an additive saw bass line through an envelope-swept filter, crushed hats, and a dotted-eighth
delay send — to `sparq-demo.wav` in about a third of a second, with no audio device involved. Copy the
WAV to your local machine and listen.

```bat
scripts\demo.bat --bars 8 --bpm 160                          longer, faster
scripts\demo.bat --crush-bits 6                              crushed hats
scripts\demo.bat --delay-send 0.6                            wetter
scripts\demo.bat --list-patterns                             every style and mutation preset
scripts\demo.bat --pattern breakcore --mutation grid-break --seed 7
scripts\demo.bat --pattern glitch --mutation chaos --seed 3 --show-dna
```

Four styles (`techno`, `breakcore`, `glitch`, `driving`) and eleven mutation presets (`grid-break`,
`densify`, `canon`, `chaos`, `structure`, `sierpinski`, `markov-walk`, `euclidean-drift`, `roll`,
`logistic`, `stutter`). **Every distinct seed gives a different loop**, and the same seed always
gives the same one — so `--seed 0` is the unmutated preset and any other seed is a variation you can
return to exactly.

`--show-dna` prints the mutation lineage and a **generative score**: the operator chain and seeds that
reproduce the render. That is the navigable form of "how did I get this rhythm", and the thing that
turns random exploration into directed exploration.

Because the render is deterministic, the printed `hash` identifies the audio exactly: same settings
and seed produce a bit-identical file. That makes the hash a stronger regression check than
listening, and it is what `reference/golden/phaseb-demo/manifest.txt` pins.

### Verifying audio without a device

This is the workflow that actually works over RDP, and it is the one CI uses:

```bat
target\release\sparq.exe render --out check.wav --seconds 10 --rate 44100 --freq 220 --gain -12
```

Then play `check.wav` anywhere — on the RDP client, on your laptop, in a DAW. Because the render is
deterministic, its hash is a stronger statement than "it sounded right":

```bat
target\release\sparq.exe selftest --golden     &REM compares against the committed reference hash
```

**Rule for the rest of Phase 0:** DSP correctness is verified by offline render + golden hash.
Latency, underruns, exclusive mode, ASIO and multichannel are verified at the physical machine, and
are *not* claimed as done until they are.

---

## The shell prototype (WO-012): `scripts\ui.bat`

```bat
scripts\ui.bat                    open the shell window
scripts\ui.bat --contrast         open in the high-contrast theme (also togglable in-window: HC button)
scripts\ui.bat --audit            the layout gate, headless: breakpoint x DPI x mode matrix,
                                   44 px floor, DPI invariance, synthetic-gesture smokes
scripts\ui.bat --headless 120     headless frame statistics (no window)
```

What works with a mouse (or a forwarded touch screen): tapping every control, panel
collapse/expand, edge swipes from the left/right/bottom screen edges, long-press (hold still
~0.35 s) for the context log, and canvas drags. The multi-touch gestures (two-finger pan/pinch,
the ×10 fine drag, 3-finger undo, 5-finger recovery) need a real touch device — a mouse is a
single pointer — but `--audit` runs them all synthetically through the same recogniser.

**Over RDP** the window opens through wgpu's WARP software rasteriser: everything works, redraws
are slower. The 60 fps number belongs to the physical machine. **Over RDP with touch**: only if
your RDP client forwards touch; otherwise use the mouse (same recogniser, one pointer).

**What I need back from the shell** (details + table in `docs/ui/windows-dpi-notes.md` §4):
the DPI matrix at 100/125/150/200 % (do taps land on the visuals?), mixed-DPI monitor behaviour,
and — if you have any touch device — one honest finger-test of tap / drag / long-press /
two-finger pan. `logs\ui.log` records startup failures; the on-canvas log records every gesture
and every *suppressed* input, so "my touch did nothing" is always explainable from the screen.

---

## What you're actually running (and what you aren't)

**Real and working:**

* A 4-crate Rust workspace with 48 passing tests, `clippy -D warnings` clean.
* The signal chain `syn/sine → util/gain → out`, with `f64` phase accumulation and dB gain smoothing.
* A block-based engine with a lock-free control ring: parameter changes reach the audio path at block boundaries, never mid-block.
* **Zero allocations on the audio path**, measured — not assumed. `scripts\gates.bat` proves it, and the probe prints it.
* Deterministic offline render: the same config twice gives bit-identical output, verified against a golden reference hash.
* A null/virtual device, so all of the above is testable with no sound card at all (which is how it was developed).
* `sparq render` writes WAV up to 64 channels; `sparq soak` runs multi-hour reliability gates; `sparq selftest` prints the Phase 0 gate table.

**Not there yet — deliberately:**

* A **GUI shell prototype** (WO-012 increment 1): `scripts\ui.bat` opens the token-styled shell — rail / top bar / canvas / inspector / dock — with the full §14.3 gesture model (tap, drag, long-press context, two-finger pan/pinch, ×10 fine drag, 3-finger undo, 5-finger recovery) driving it. It renders placeholders: the graph canvas is WO-013, the engine bindings are WO-007/008. The audio engine is still terminal-controlled.
* No sequencer, no samples, no effects beyond gain. Phases B and E.
* No WASAPI **exclusive** mode, no **ASIO**, no MMCSS, no working-set lock, no multichannel aggregation, no real latency measurement. That is WO-006, and it needs your hardware to verify.
* The live path is the **disposable bootstrap** (ADR-008): shared mode, a mutex in the callback, `cpal` as the device layer. It is scheduled for deletion, and its two policy violations are marked in the source with the work order that removes them.

---

## What I need back from you

Run `scripts\diag.bat` and send me **`sparq-diag.log`** — whether or not it worked. It contains your device list, the format negotiation, the callback telemetry and the selftest results. With that I can:

1. confirm the sample rate and channel count to target,
2. see whether your interface needs ASIO or behaves under WASAPI,
3. pick the right block size before I write the real HAL (WO-006),
4. and know whether Phase A actually passed on real hardware — which I cannot verify from here.

Also useful, whenever you have them: your interface model, whether you have a touchscreen device yet, and Windows version (`winver`).
