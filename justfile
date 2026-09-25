# sparq — developer tasks.
# `just` is optional: every recipe is a plain command you can copy. `cargo xtask` wraps the same
# commands for people who prefer not to install another tool.

default: gates

# Everything CI runs, in order. This is the pre-push command.
gates: fmt-check clippy test python-gates golden golden-demo selftest ui-audit

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p sparq-app --features bootstrap-audio --all-targets -- -D warnings

# The WO-006 gate that makes blind WASAPI development honest: the Windows-only HAL backend must
# compile and lint for the MSVC target from every machine, including Linux sandboxes. Needs
# `rustup target add x86_64-pc-windows-msvc` once. Does not link — no MSVC toolchain required.
check-windows:
    # WO-007: the contract crate is pure Rust, so its MSVC cell is cheap and catches cfg drift early.
    cargo clippy --target x86_64-pc-windows-msvc -p sparq-module-api --all-targets -- -D warnings
    cargo clippy --target x86_64-pc-windows-msvc -p sparq-kernel --features hal-wasapi --all-targets -- -D warnings
    cargo clippy --target x86_64-pc-windows-msvc -p sparq-app --features hal-wasapi --all-targets -- -D warnings
    cargo clippy --target x86_64-pc-windows-msvc -p sparq-kernel --all-targets -- -D warnings
    cargo clippy --target x86_64-pc-windows-msvc -p sparq-app --all-targets -- -D warnings
    # WO-012: the window stack must lint for MSVC too (wgpu picks the dx12 backend there).
    # Heavy (the `windows` crate); this is the cell that protects the SATURN build blind.
    cargo clippy --target x86_64-pc-windows-msvc -p sparq-app --features ui-window --all-targets -- -D warnings

# WO-012 UI cells. `check-ui` lints both halves of the shell: the hermetic egui half (`ui`) and
# the window host (`ui-window`, which on this host compiles the gles backend — the sandbox-fit
# choice; SATURN compiles the same window.rs against dx12 via check-windows).
check-ui:
    cargo clippy -p sparq-app --features ui --all-targets -- -D warnings
    cargo clippy -p sparq-app --features ui-window --all-targets -- -D warnings

# The WO-012 gate: breakpoint x DPI x mode matrix + 44 px floor + gesture smoke, headless.
ui-audit:
    cargo run -q --release -p sparq-app --features ui -- ui --audit

# A short headless run: frame-time statistics for layout+draw+tessellate (no GPU involved).
ui-headless:
    cargo run -q --release -p sparq-app --features ui -- ui --headless 120

test:
    cargo test --workspace

# The three Python gates: tokens up to date, mockups/source conformant, unsafe allowlisted.
python-gates:
    python3 tools/token_gen.py --check
    python3 tools/token_audit.py
    python3 tools/unsafe_audit.py

# Regenerate the token artefacts after editing design/tokens/*.toml.
tokens:
    python3 tools/token_gen.py

# Regenerate the display-sheet mockup after a token change, then re-audit.
mockups:
    python3 tools/make_display_sheet.py
    python3 tools/token_audit.py

build:
    cargo build --workspace

release:
    cargo build --release --workspace

# Render the golden reference and print the manifest lines to paste into
# reference/golden/wo005/manifest.txt. Regeneration is EXPLICIT and reviewed (ADR-007):
# never do this just to make a test pass.
golden:
    cargo run -q -p sparq-app -- golden-values
    @echo "--- update reference/golden/wo005/manifest.txt with the values above, with a reason ---"

# The Phase 0 gate table, measured in-process.
selftest: release
    ./target/release/sparq selftest --golden

# Prove the audio path makes zero allocations, outside the test harness.
probe: release
    cargo run -q --release -p sparq-app --example probe_alloc

# Render the Phase B patch (kick/bass/hat/delay) to WAV. No audio device needed - this is the
# path that works over RDP, and it is deterministic so a change can be A/B'd by hash.
demo out="sparq-demo.wav": release
    ./target/release/sparq demo --out {{out}}

# Regenerate the Phase B golden reference values (then update the manifest with a reason).
golden-demo:
    cargo run -q --release -p sparq-app -- demo --bpm 138 --bars 4 --rate 48000 --ch 2 --seed 0xA17E --golden-values

# Render a scratch file for listening.
render out="out/local.wav" seconds="10": release
    mkdir -p out
    ./target/release/sparq render --out {{out}} --seconds {{seconds}}

# Live output through the disposable bootstrap path (ADR-008). Deleted at end of WO-006.
play:
    cargo run -p sparq-app --features bootstrap-audio -- play

# Live output through the real HAL (WO-006). On Windows add `--features hal-wasapi` and
# `--backend wasapi-exclusive`; the null backend works everywhere and needs no features.
hal-devices:
    cargo run -q -p sparq-app -- devices --caps

hal-play-null seconds="5":
    cargo run -q -p sparq-app -- play --backend null --seconds {{seconds}}

hal-conformance:
    cargo run -q -p sparq-app -- devices --conformance

# Long-run reliability gate (WO-016). Bounded memory: renders in 10 s chunks and discards them,
# while accumulating counters and re-checking determinism every chunk. Fails fast on the first
# allocation or underrun, because those are defects, not statistics.
soak minutes="120": release
    ./target/release/sparq soak --minutes {{minutes}}

# A short soak, suitable for CI and for a pre-flight check on the stage machine.
soak-short: release
    ./target/release/sparq soak --minutes 2 --report-every 20

# The full Phase 0 exit gate.
phase0: gates soak-short
    @echo "--- Phase 0 gate table: PHASE0-WORKORDERS.md §4. The 2 h / 12 h soaks need the stage machine. ---"

clean:
    cargo clean
