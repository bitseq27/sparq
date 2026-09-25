# sparq module author guide — v0

**Audience:** you, writing a module for sparq. **Status:** v0, matching `sparq-module-api` as built in
WO-007. **The test this document has to pass** (module-api §15): a simple module is a **two-hour**
task and a complex one is a two-day task. If any step below needs engine knowledge, the contract has
failed and *this document gets amended* — that is the rule, not a courtesy.

Everything here is worked from the three reference modules that shipped with the contract:
`util/gain`, `syn/sine` and `ana/rms`, in `crates/sparq-module-api/tests/contract.rs`. They are small
on purpose. Read them before reading further; this guide explains what they already demonstrate.

---

## 1. What a module is

Five things. Three are code, two are paperwork, and the paperwork is not optional:

| Part | Required | Notes |
|---|---|---|
| `sparqmod.toml` | yes | The contract's front half. Validated at discovery; a bad manifest never loads |
| Implementation | yes | The `Module` trait: `prepare` + `process`, plus three methods with defaults |
| Golden render | yes (T1) | `reference/golden/<id>/<case>.wav` + its expected hash |
| Unit tests | yes | The properties from module-api §13 that apply to your module |
| Example patch | yes | A `.sparqpatch` showing the module doing its job |
| Panel descriptor | **no** | Absent, the shell generates a panel from your parameter list |
| Docs | generated | Produced *from the manifest*. Do not hand-write a duplicate |

## 2. The two-hour path

1. Copy the manifest in §5 and change the ids.
2. Implement `prepare` and `process` in one file. Everything else has a default.
3. Validate before you write any DSP: `Registry::register(MANIFEST, factory)` returns every failure
   at once, with the field, the value found, the values allowed, and the fix.
4. Add the golden render and the real-time test (§10).
5. Drop it in `modules/`. It is discovered; nothing in the engine is edited. **That last claim is
   machine-checked**: `tests/throwaway_module.rs` adds a module that appears nowhere else in the
   repository and then walks every `.rs` under every `crates/*/src` asserting none of them mentions
   it. If adding your module ever requires an engine edit, that test names the file.

## 3. What you must implement

```rust
impl Module for MyModule {
    fn id(&self) -> &str { "sparq/util/my-module" }        // must equal identity.id, exactly

    fn prepare(&mut self, r: &Resources) -> Result<(), ModuleError> {
        // ALL allocation happens here. Buffer sizes, tables, arenas, voice pools.
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // No allocation. No locking. No clock. No I/O. No panics. See §6.
        BlockStatus::Ok
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> { Ok(()) }
    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> { Err(ModuleError::Message("none")) }
}
```

`activate` and `deactivate` have defaults. `configure` and `message` do not, but refusing honestly is
a valid implementation — `ana/rms` returns `Err` because it takes no messages, and a module that
silently ignores a message it does not understand is a module whose author will spend a day finding
out why nothing happened.

**Your id is checked at registration, not at first use.** `Registry::register` builds your module once
and compares `id()` against `identity.id`; a disagreement is a registration refusal. A module that
lies about who it is breaks patch provenance silently — the project file records the id, and the wrong
code runs.

**`prepare` is called again whenever anything in `Resources` changes**: sample rate, block size,
channel sets, oversampling, voice count, arena budget. A channel-set *resolution* counts, and it
cascades — when one module's `variable` port resolves, every module whose effective channel set
depends on it is re-prepared in topological order, in the same control-thread pass, without touching
the graph's version or mutation counter. Write `prepare` to be idempotent and to release what it
previously took.

## 4. What you get free

* **Parameter delivery.** A `Copy` snapshot swapped at the block boundary, over a lock-free ring. You
  read `ctx.param(i)`; you never see a half-written value, and a change made mid-block reaches you at
  the next boundary. Tested, not promised.
* **Smoothing.** The host smooths in the audio thread unless you declare `self_smoothed`.
* **Buffers.** Contiguous, aligned, valid for the whole block, sized exactly
  `frames × channels`.
* **Validation and its messages.** 24 error kinds, each naming the field and the fix.
* **Discovery and precedence**, the module browser's generated docs, the auto-generated panel.
* **The watchdog.** Exceed your declared block budget N times consecutively and the executor
  auto-bypasses you, draws a red hairline and writes a journal entry. Lying about `cpu_class` is
  visible in the diagnostics panel — that is the point, not a punishment.
* **Latency accounting** per path, with a global raw/compensated switch (ADR-006: latency is a timbral
  resource, so compensation is opt-in, never automatic).
* **A seeded RNG** from the kernel's seed tree, so your randomness replays identically.

## 5. The manifest, in full, for a real module

```toml
[identity]
id = "sparq/util/gain"          # stable forever; renaming needs an `aliases` entry
version = "0.1.0"
host_api = { min = 1, max = 1 }
display_name = "Gain"
summary = "Gain and trim"        # one line, ≤ 120 characters: tooltips and search use it
authors = ["sparq"]
license = "MIT"

[classification]
category = "utility/gain"
top = "util"                     # closed set: syn smp flt fx dyn ana spa seq gen harm dat io dsp util ml out
kind = "processor"               # closed set: source processor utility analysis spatial display data generative io
tier = "t1"                      # t1 native · t2 wasm (Phase 5) · t3 process
stability = "stable"

[[ports]]
id = "in"
name = "Input"
direction = "in"
type = "audio"                   # closed set of six: audio cv event data gpu atom (ADR-005)
channel_set = "stereo"           # or "variable", resolved at prepare

[[ports]]
id = "out"
name = "Output"
direction = "out"
type = "audio"
channel_set = "stereo"

[[params]]
id = "gain"
name = "Gain"
type = "float"
unit = "ratio"                   # closed vocabulary; there is no "dBFS" and no invented unit
min = 0.0
max = 2.0
default = 1.0                    # must be inside [min, max]
smoothing = "one_pole:5"         # host-applied unless self_smoothed = true

[state]
schema_id = "sparq/util/gain/state"   # mandatory: a module that cannot save cannot be in a project
schema_version = 1

[resources]
latency = 0                      # or "param:<id>" when it follows a parameter
cpu_class = "trivial"
```

**Every required field is required.** 22 of them, and an absent one is `E-KEY-MISSING:<path>` naming
the exact path. An unknown key is also an error (`E-UNKNOWN-KEY`) — typo protection, and it applies
inside sections v0 does not yet decode, so `colour_klass` inside `[ui]` is refused rather than
silently ignored.

## 6. What you must never do inside `process`

The ten rules of module-api §9, in author terms:

1. **Allocate or free.** No `Box`, `Vec`, `String`, `format!`, no closure that captures by value into
   the heap. Everything you need was reserved in `prepare`.
2. **Lock.** No `Mutex`, no `RwLock`, no `Rc`. These are denied by `clippy.toml` workspace-wide, so
   this fails the build rather than the performance target.
3. **Touch the OS.** No file, network, GPU-submit, IPC or subprocess.
4. **Read the clock.** `Instant::now` and `SystemTime::now` are banned workspace-wide. Use the block's
   timestamp from `ctx.block`.
5. **Use unseeded randomness.** All RNG comes from the seed tree, or your module does not replay.
6. **Depend on hash-map iteration order.** It is not stable, and neither is your golden render then.
7. **Panic.** Report through `BlockStatus`; never unwind across the boundary.
8. **Read or write outside the buffers you were given.**
9. **Assume denormal flushing is off.** It is on: denormals-are-zero on the audio path.
10. **Exceed your declared block budget.**

The enforcement is layered, and it is worth knowing which layer is which, because one of them is
weaker than it looks. `AudioCtx` **offers no capability** — no allocator, no clock, no filesystem, no
lock — so the rule is the absence of an API rather than a comment. `clippy.toml` denies the specific
types and calls. And the claim is **measured**: `tests/contract.rs` installs the kernel's counting
allocator as its global allocator and asserts **0 allocations across 15 000 `process` calls through
`Box<dyn Module>`**, after an un-measured warm-up so the harness's own lazy initialisation is not
counted as yours. What Rust *cannot* do is forbid `Vec::new()` by types alone, so the honest statement
is "no capability offered, no allocation measured" — not "impossible". Write your module as if it were
impossible, and the test will tell you if it isn't.

## 7. Parameters

Every parameter is a modulation target: any `cv` output, `data` channel, event CC or LFO can drive it,
with amount, curve, bipolar and smoothing. That is one system, not three, and you do not implement any
of it — you declare `min`, `max`, `default`, `unit` and `curve`.

Two declarations authors get wrong:

* **`morph = "discrete"`** for a parameter that cannot be interpolated meaningfully — a mode switch,
  an enum, a table index. Scene crossfades, preset morphs and DNA-tree auditioning all interpolate
  between two parameter vectors; a module that cannot interpolate a parameter declares it, and the
  host holds it constant during a morph instead of interpolating garbage. Default is `continuous`,
  so silence here means "interpolate me".
* **`per_voice = true`** requires `voices.policy` to be something other than `none`. This is a
  cross-field rule with its own error, not a warning.

v0 caps a module at **32 parameters**, because the snapshot is `Copy` and travels through a ring slot
without allocating. A manifest over the cap is refused with an explanation, not truncated.

## 8. Ports and what connects

Six types, closed by ADR: adding one requires a new ADR *and* a host major version. New kinds of
content are subtypes or `atom` payloads — MIDI and OSC are both `event`, and the payload carries the
dialect.

You do not implement connection logic; the host does, from one table
(`docs/api/compat-matrix.toml`). What you declare affects it:

* `channel_set = "variable"` means "tell me at `prepare`". Nothing is silently up/downmixed except the
  two documented cases: `mono → multi` fans out, and `multi → mono` **sums and draws a warning
  hairline**.
* `ambisonics:*` refuses to connect to a non-spatial port unless an encoder or decoder is inserted.
* `cv` fan-out is free; fan-in requires an explicit merge module — there is no implicit summing.
* A `cv` **range** mismatch (bipolar into unipolar) is refused and offers `util/range`. It is never
  silently rescaled: an invisible transformation of your sound is exactly what the type system exists
  to prevent.
* **Rate mismatches are yours to declare.** An audio-rate source feeding your block-rate input is
  reduced by your `cv_reduce` (`last`, `first`, `mean`, `min`, `max`, `peak`; default `last`). A
  block-rate source feeding your audio-rate input is expanded by the host per your `cv_interp`
  (`hold`, `linear`, `spline`; default `hold`). These live in the manifest and never in code, because
  `first` and `last` produce *different audio* — an undeclared choice would make a journal replay
  diverge from the original render.
* A converter is only offered if it exists. `data → cv` is refused until `dat/mapper` ships in
  Phase 5, rather than presenting a button that does nothing.

## 9. State, assets, latency

State is a versioned, serialisable blob — **no exceptions**, because a module that cannot save its
state cannot be in a project. Declare `schema_id` and `schema_version`; declare migrations as an
ordered chain with no gaps, and loading a 2027 patch in 2031 runs the chain, re-saves, and preserves
the original. Reference assets **by hash, never by path**, so projects stay portable and verifiable.

Declare latency honestly, including `0`. `E-LATENCY-UNDECLARED` exists for the case where measured
latency is non-zero and declared latency is zero, and the executor checks it — latency compensation
and the whole "latency is a timbral resource" position depend on the declared number being true. When
latency follows a parameter (convolution size, grain count), declare `latency = "param:<id>"` and the
executor recomputes on change; a parametric latency is not judged statically against a measurement,
because there is no constant to contradict it.

## 10. Testing your module

Four tests, and the harness for each already exists:

1. **Golden render** — deterministic, hashed, tolerance documented. Two renders from the same state
   and seed must be bit-identical; `syn/sine` proves its phase wraps rather than grows by integrating
   200 whole periods to a mean under 1e-6.
2. **Real-time discipline** — zero allocations in `process` under the counting allocator.
3. **State round-trip** — save, load, identical output; plus migration from the previous schema
   version.
4. **Properties that apply to you** — DC-free if AC-coupled, bandlimited within declared tolerance,
   no denormal stalls on worst-case input, silence in ⇒ silence out, latency and tail equal to
   declared values (*measured, not asserted*), parameter extremes produce no NaN or Inf.

Choose test frequencies that make your property exactly true. `syn/sine`'s phase test first ran at
440 Hz — 109.09 frames per period at 48 kHz — over whole 64-frame blocks, which is never a whole
number of periods, so the mean could not cancel and the test measured its own arithmetic instead of
phase drift. At 750 Hz one block *is* one period and the property is exact. A test that fails for the
wrong reason and is then loosened until it passes is how a gate quietly stops testing anything.

## 11. v0 limits — what you cannot do yet

* **One interleaved audio input and one output.** Multi-port buffers arrive with the WO-008 executor,
  which owns buffer pooling and per-path latency.
* **No sub-block processing.** A module may not chop the host block into finer internal passes; there
  is no `internal_rate` field. Grain and event **onsets are still sample-accurate** — events arrive
  pre-sorted by sample offset and you render into the block buffer, so a granular cloud can start a
  grain at sample 37 of 64. What v0 gives up is parameter *movement* inside a block. If you genuinely
  need finer resolution, the remedy is a smaller **host** block (`BlockContext.frames` is a runtime
  field, and 100 modules at 16 frames costs ~0.52 % of a core against ~0.13 % at 64), not a contract
  change.
* **`data` and `gpu` payloads are declared and validated but not yet carried**, and `event` payloads
  are declared but not carried. `gpu` ports are first-party T1 only until Phase 5 decides the tier
  question; `ana/tap` and `dsp/scope` are the Phase-0 modules that use them.
* **No hot reload** (Phase 1), **no wasm tier** (Phase 5), **no state migration chains beyond the
  declaration** — the schema id and version fields exist now so migration can be added without
  breaking manifests.
* **Panels are auto-generated** unless you declare one; the widget vocabulary and the ≥ 44 px touch
  audit are enforced by `sparq-ui`, which already ships.

## 12. Where the truth lives

| Question | Authority |
|---|---|
| What may connect to what | `docs/api/compat-matrix.toml` — data, and the canvas renders it directly |
| Every manifest field, its domain, its error code | `docs/api/manifest-fields.toml` |
| Why the contract is shaped this way | `docs/api/module-api-v1.md`, ADR-005 (+ addendum), ADR-002, ADR-009 |
| What the engine actually enforces | `crates/sparq-module-api/src`, and `tests/api_snapshot.rs` which pins every public signature |
| A worked module | `crates/sparq-module-api/tests/contract.rs` — `util/gain`, `syn/sine`, `ana/rms` |

Where this guide and the tables disagree, **the tables win and this guide is wrong**. Say so and it
gets fixed; a document that argues with the data is how authors end up trusting the document.
