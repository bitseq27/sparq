//! The `sparq ui` surface (WO-012): the egui shell prototype.
//!
//! Two hosts share one shell:
//!
//! * **window** (`--features ui-window`): winit + egui-winit + egui-wgpu — the real thing, for
//!   the DPI/touch/60 fps acceptance runs on SATURN.
//! * **headless** (`--features ui`): drives `egui::Context` directly with no window and no GPU —
//!   the null-device of the UI. The layout audit, the breakpoint matrix and the synthetic-gesture
//!   smoke tests all run here, which means CI proves the shell's *logic* on every commit and the
//!   hardware runs only have to prove the hardware parts (DPI mapping, touch digitiser, fps).
//!
//! Both call the same [`ShellUi::frame`]; neither the gesture recogniser nor the layout model
//! knows which host is driving. That is the ADR-003 bargain: egui is the drawing half, and the
//! drawing half is replaceable.

#[cfg(feature = "ui")]
pub mod adapter;
#[cfg(feature = "ui")]
pub mod canvas_ui;
#[cfg(feature = "ui")]
pub mod headless;
#[cfg(feature = "ui")]
pub mod live;
#[cfg(feature = "ui")]
pub mod shell_ui;
#[cfg(feature = "ui-window")]
pub mod window;

/// Parsed `sparq ui` options (cli.rs owns argument parsing; this is the typed result).
#[derive(Clone, Debug)]
pub struct UiOptions {
    /// Run N frames headless (no window), then exit. 0 = window mode.
    pub headless_frames: usize,
    /// Run the full audit matrix (implies headless) and gate on it.
    pub audit: bool,
    /// Logical viewport width for headless runs.
    pub width: f32,
    /// Logical viewport height for headless runs.
    pub height: f32,
    /// Pixels-per-point (DPI scale) for headless runs.
    pub scale: f32,
    /// Start in the high-contrast theme.
    pub contrast: bool,
    /// Dump the final headless frame's vector shapes to an SVG file (the visual-regression
    /// instrument: a screenshot without a GPU — WO-012 increment 3).
    pub svg_out: Option<String>,
    /// With `svg_out`: dump the convergence-review state instead of the bare demo patch —
    /// slice B's own showcase (WO-012 increment 5): an `env/ad`, a `mod/lfo` and an `flt/svf`
    /// spawned beside the demo chain, the svf SELECTED with its probe marker at the mockup's
    /// own 1 240.0 Hz, and a pumped manual-null session so the meter wells read live. The
    /// instrument for `design/mockups/convergence-wo012-inc5.png`.
    pub review: bool,
}

impl Default for UiOptions {
    fn default() -> Self {
        Self {
            headless_frames: 0,
            audit: false,
            svg_out: None,
            review: false,
            width: 1920.0,
            height: 1080.0,
            scale: 1.0,
            contrast: false,
        }
    }
}

/// Entry point for `sparq ui`. Returns a process exit code.
#[must_use]
pub fn run(opts: UiOptions) -> i32 {
    #[cfg(feature = "ui")]
    {
        if opts.audit {
            return headless::run_audit(&opts);
        }
        if opts.svg_out.is_some() {
            return headless::run_svg(&opts);
        }
        if opts.headless_frames > 0 {
            return headless::run_frames(&opts);
        }
        #[cfg(feature = "ui-window")]
        {
            window::run(&opts)
        }
        #[cfg(not(feature = "ui-window"))]
        {
            let _ = opts;
            eprintln!("sparq ui: window mode needs the windowing stack.");
            eprintln!("  build it with:  cargo build --release -p sparq-app --features ui-window");
            eprintln!("  (scripts\\build.bat does this automatically on Windows)");
            eprintln!("  headless works now:  sparq ui --headless 60   |   sparq ui --audit");
            2
        }
    }
    #[cfg(not(feature = "ui"))]
    {
        let _ = opts;
        eprintln!("sparq ui: this binary was built without the UI feature.");
        eprintln!("  build it with:  cargo build --release -p sparq-app --features ui-window");
        eprintln!("  (the default build is deliberately dependency-free: ADR-003/ADR-008)");
        2
    }
}
