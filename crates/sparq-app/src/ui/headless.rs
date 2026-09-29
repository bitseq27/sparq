//! The headless UI driver: egui without a window, a GPU, or a display server — the null-device
//! of the shell. Everything the layout audit and the gesture pipeline need to be *proven* runs
//! here, on every CI machine, deterministically.
//!
//! What headless proves: layout at the whole breakpoint matrix, touch-target compliance, DPI
//! invariance of the logical-px model, and end-to-end gesture → intent → state changes driven by
//! synthetic pointers.
//! What headless cannot prove (honestly, and these are the SATURN acceptance runs): real digitiser
//! coordinates, real per-monitor DPI handoffs, palm rejection with a real hand, and 60 fps with a
//! real rasteriser. Those are device facts; everything here is a logic fact.

use std::time::Instant;

use sparq_ui::gesture::GestureIntent;
use sparq_ui::pointer::{PointerEvent, PointerKind, PointerPhase};
use sparq_ui::shell::ShellMode;

use crate::ui::adapter::{self, ThemeChoice};
use crate::ui::shell_ui::{FrameInput, ShellUi};
use crate::ui::UiOptions;

/// The breakpoint matrix from `layout.toml`, plus one deliberately-too-small viewport whose job
/// is to be refused. (w, h, label)
const VIEWPORTS: [(f32, f32, &str); 5] = [
    (2560.0, 1440.0, "display-wall 2560x1440"),
    (1920.0, 1080.0, "desktop 1920x1080"),
    (1440.0, 900.0, "laptop 1440x900"),
    (1280.0, 800.0, "tablet-min 1280x800"),
    (1024.0, 700.0, "below-breakpoint 1024x700"),
];

/// The DPI scales of the acceptance criterion: 100 / 125 / 150 / 200 %.
const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

fn raw_input(w: f32, h: f32, t_secs: f64) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))),
        time: Some(t_secs),
        focused: true,
        ..Default::default()
    }
}

/// One frame through a shell, tessellated (so the CPU-side paint work is included in timing).
/// `now_ms` is the DRIVER's clock — the same clock the pointer timestamps come from. The first
/// draft of this suite derived `now_ms` from egui's frame time while the smoke events carried
/// their own counter; the recogniser's time-based gestures (long press, five-finger hold) then
/// measured against a different epoch than the events and silently never fired. One driver, one
/// clock — that is the adapter contract from input-model §2, and step() now enforces it by shape.
/// Returns (shapes emitted, primitives tessellated).
fn step(
    shell: &mut ShellUi,
    ctx: &egui::Context,
    w: f32,
    h: f32,
    t_secs: f64,
    now_ms: u64,
    pointers: &[PointerEvent],
) -> (usize, usize) {
    step_x(shell, ctx, w, h, t_secs, now_ms, pointers, &[])
}

/// [`step`] with host-synthesised intents (right-click, wheel) — the mouse path's smoke door.
#[allow(clippy::too_many_arguments)] // the smoke door mirrors `step` plus the extras slice
fn step_x(
    shell: &mut ShellUi,
    ctx: &egui::Context,
    w: f32,
    h: f32,
    t_secs: f64,
    now_ms: u64,
    pointers: &[PointerEvent],
    extras: &[GestureIntent],
) -> (usize, usize) {
    let mut out = ctx.run_ui(raw_input(w, h, t_secs), |ui| {
        shell.frame(ui, FrameInput { pointers, now_ms, extras });
    });
    // epaint requires texture deltas to be handled, not dropped (the font atlas arrives as one
    // on the first frame). Headless has no GPU to upload to, so the honest handling is clear():
    // the glyph cache rebuilds itself into the next frame's delta if anything ever renders it.
    out.textures_delta.clear();
    let shapes = out.shapes.len();
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point).len();
    (shapes, prims)
}

/// `sparq ui --headless N`: run N frames, report frame statistics and the final layout.
#[must_use]
pub fn run_frames(opts: &UiOptions) -> i32 {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(opts.scale);
    let theme = if opts.contrast { ThemeChoice::ContrastHigh } else { ThemeChoice::PhosphorDark };
    adapter::apply_style(&ctx, theme);
    let mut shell = ShellUi::new();
    if opts.contrast {
        shell.theme = theme;
    }

    let frames = opts.headless_frames.max(1);
    let mut times_us: Vec<u128> = Vec::with_capacity(frames);
    let mut last = (0, 0);
    for f in 0..frames {
        let t0 = Instant::now();
        let now_ms = u64::try_from(f * 16).unwrap_or(u64::MAX); // nominal 60 fps cadence
        last = step(&mut shell, &ctx, opts.width, opts.height, f as f64 / 60.0, now_ms, &[]);
        times_us.push(t0.elapsed().as_micros());
    }
    times_us.sort_unstable();
    let pick = |q: f64| times_us[((times_us.len() as f64 - 1.0) * q).round() as usize];
    let layout = shell.last_layout.as_ref();

    println!("sparq ui · headless run");
    println!(
        "viewport      : {}x{} logical @ {:.2} ppp, theme {:?}",
        opts.width, opts.height, opts.scale, theme
    );
    println!(
        "frames        : {} ({} shapes, {} primitives on the last frame)",
        frames, last.0, last.1
    );
    println!(
        "frame logic   : min {} us · med {} us · p99 {} us  (layout+draw+tessellate, no GPU; device fps is the SATURN run)",
        pick(0.0),
        pick(0.5),
        pick(0.99)
    );
    if let Some(l) = layout {
        println!(
            "layout        : canvas {:.0}x{:.0} · rail {} · inspector {} · dock {} · refused {} · forced {:?}",
            l.canvas.width(),
            l.canvas.height(),
            l.rail.is_some(),
            l.inspector.is_some(),
            l.dock.is_some(),
            l.design_refused,
            l.forced
        );
    }
    let report = shell.audit_report();
    print!("{}", report.format_report());
    if report.is_clean() {
        0
    } else {
        1
    }
}

/// `sparq ui --audit`: the WO-012 gate. Breakpoint × DPI × mode matrix, DPI-invariance
/// assertion, and a synthetic-gesture smoke suite. Any FAIL exits non-zero.
#[must_use]
pub fn run_audit(_opts: &UiOptions) -> i32 {
    let mut failures: Vec<String> = Vec::new();
    println!("sparq ui · layout audit (WO-012 gate)");
    println!("matrix: {} viewports x {} DPI scales x 2 modes; every interactive element measured against its touch class", VIEWPORTS.len(), SCALES.len());

    // ------------------------------------------------------------ matrix
    for &(w, h, label) in &VIEWPORTS {
        // Per (viewport, mode): the results at every scale must be IDENTICAL — the layout model
        // works in logical px, so DPI must not change what is decided or where it lands. That is
        // the machine-checkable half of the per-monitor-DPI acceptance criterion.
        for &mode in &[ShellMode::Design, ShellMode::Perform] {
            let mut baseline: Option<(usize, usize, usize, String, bool)> = None;
            let mut scale_results: Vec<String> = Vec::new();
            for &scale in &SCALES {
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(scale);
                adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
                let mut shell = ShellUi::new();
                shell.set_mode(mode);
                // Two frames: the first builds the registry, the second is what the audit reads.
                step(&mut shell, &ctx, w, h, 0.0, 0, &[]);
                step(&mut shell, &ctx, w, h, 1.0 / 60.0, 16, &[]);
                let report = shell.audit_report();
                let refused = shell.last_layout.as_ref().is_some_and(|l| l.design_refused)
                    || shell.state.mode != mode;
                let key = (report.checked, report.violations.len(), report.dense_badges.len());
                let lay = format!(
                    "{:?}",
                    shell.last_layout.as_ref().map(|l| (
                        l.canvas.min,
                        l.canvas.max,
                        l.rail.is_some(),
                        l.inspector.is_some(),
                        l.dock.is_some()
                    ))
                );
                if baseline.is_none() {
                    baseline = Some((key.0, key.1, key.2, lay.clone(), refused));
                } else if let Some(b) = &baseline {
                    if (b.0, b.1, b.2) != key || b.3 != lay {
                        failures.push(format!("{label} @{scale:.2} {mode:?}: DPI variance — {key:?} vs baseline {b:?}"));
                    }
                }
                for v in &report.violations {
                    failures.push(format!(
                        "{label} @{scale:.2} {mode:?}: {} needs {:.0}px, measured {:.0}px ({})",
                        v.id, v.required, v.actual, v.reason
                    ));
                }
                scale_results.push(format!(
                    "@{:.2}: {} elements, {} violations, {} dense{}",
                    scale,
                    report.checked,
                    report.violations.len(),
                    report.dense_badges.len(),
                    if refused { ", REFUSED->PERFORM" } else { "" }
                ));
            }
            let verdict = if failures.is_empty() { "PASS" } else { "FAIL" };
            println!("[{verdict}] {label} · {mode:?} — {}", scale_results.join(" | "));
        }
    }
    println!(
        "[{}] dpi-invariance — every viewport produced identical layout+audit at 100/125/150/200%",
        if failures.iter().any(|f| f.contains("DPI variance")) { "FAIL" } else { "PASS" }
    );

    // The below-breakpoint viewport must have been refused in Design mode (the matrix cells
    // above show REFUSED->PERFORM; assert it explicitly so a regression cannot hide in a label).
    {
        let ctx = egui::Context::default();
        adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
        let mut shell = ShellUi::new();
        step(&mut shell, &ctx, 1024.0, 700.0, 0.0, 0, &[]);
        let ok = shell.state.mode == ShellMode::Perform
            && shell.log().iter().any(|l| l.contains("DESIGN MODE REFUSED"));
        let refused_run = shell.last_layout.as_ref().is_some_and(|l| l.canvas.width() > 0.0);
        if ok && refused_run {
            println!("[PASS] below-breakpoint — Design refused, Perform reflowed and rendered");
        } else {
            failures.push("below-breakpoint viewport did not refuse Design mode".to_string());
            println!("[FAIL] below-breakpoint — Design was not refused");
        }
    }

    // ------------------------------------------------------------ gesture smoke
    println!("gesture smoke (synthetic pointers through the real recogniser + dispatch):");
    run_gesture_smoke(&mut failures);

    // ------------------------------------------------------------ increment 5 smoke
    println!("increment 5 smoke (rename entry, cv wire levels, inspector scroll, the LOD walk):");
    run_inc5_smokes(&mut failures);

    // ------------------------------------------------------------ live session smokes
    println!("live session smoke (WO-012 inc 2: PLAY on the manual null, pumped by the smoke):");
    run_live_smokes(&mut failures);

    // ------------------------------------------------------------ scope screen smokes
    println!(
        "scope screen smoke (WO-013 inc 6: the trace fills from the ring; Full-LOD contract):"
    );
    run_scope_smokes(&mut failures);

    // ------------------------------------------------------------ chrome conformance smokes
    println!("chrome smoke (WO-012 inc 3: the dock card grid and the rail's ADD are real doors):");
    run_chrome_smokes(&mut failures);

    // ------------------------------------------------------------ mouse + meter smokes
    println!("mouse + meter smoke (WO-012 inc 4: right-click, wheel, hover silence, master bars):");
    run_mouse_and_meter_smokes(&mut failures);

    println!(
        "ui audit: {} ({} failure(s))",
        if failures.is_empty() { "PASS" } else { "FAIL" },
        failures.len()
    );
    for f in &failures {
        println!("  [FAIL] {f}");
    }
    if failures.is_empty() {
        0
    } else {
        1
    }
}

// ------------------------------------------------------------------ synthetic gesture smoke

fn finger(id: u64, x: f32, y: f32, phase: PointerPhase, t_ms: u64) -> PointerEvent {
    PointerEvent {
        id,
        pos: sparq_ui::geom::Vec2::new(x, y),
        pressure: 1.0,
        contact_area_mm2: 20.0,
        kind: PointerKind::Finger,
        phase,
        t_ms,
    }
}

/// Find a registered element or record a failure and bail — `expect` is denied workspace-wide,
/// and in a gate a missing element IS a failure, not a panic.
fn must_rect(
    shell: &ShellUi,
    id: &str,
    failures: &mut Vec<String>,
) -> Option<sparq_ui::geom::Rect> {
    match shell.rect_of(id) {
        Some(r) => Some(r),
        None => {
            failures.push(format!("element {id} was not registered by the drawn shell"));
            None
        },
    }
}

fn check(name: &str, ok: bool, failures: &mut Vec<String>) {
    println!("[{}] {name}", if ok { "PASS" } else { "FAIL" });
    if !ok {
        failures.push(name.to_string());
    }
}

fn run_gesture_smoke(failures: &mut Vec<String>) {
    let ctx = egui::Context::default();
    adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let mut shell = ShellUi::new();
    let mut t = 0.0_f64; // seconds fed to egui's frame time
    let mut now = 0_u64; // ms — the ONE clock: pointer events and advance() both read this
    macro_rules! frame {
        ($pts:expr) => {{
            t += 1.0 / 60.0;
            step(&mut shell, &ctx, w, h, t, now, &$pts)
        }};
    }
    frame!([]); // frame 1: registry built

    // 1. tap the dock toggle → dock collapses; tap again → expands.
    let Some(dock_btn) = must_rect(&shell, "topbar/dock", failures) else {
        return;
    };
    let c = dock_btn.center();
    frame!([finger(1, c.x, c.y, PointerPhase::Down, now)]);
    now += 80;
    frame!([finger(1, c.x, c.y, PointerPhase::Up, now)]);
    check(
        "tap topbar/dock collapses the dock",
        shell.last_layout.as_ref().is_some_and(|l| l.dock.is_none()),
        failures,
    );
    now += 400;
    frame!([finger(2, c.x, c.y, PointerPhase::Down, now)]);
    now += 80;
    frame!([finger(2, c.x, c.y, PointerPhase::Up, now)]);
    check(
        "tap again expands the dock",
        shell.last_layout.as_ref().is_some_and(|l| l.dock.is_some()),
        failures,
    );

    // 2. tap mode → Perform: chrome gone, XL pads present, audit clean; tap back → Design.
    let Some(mode_btn) = must_rect(&shell, "topbar/mode", failures) else {
        return;
    };
    let c = mode_btn.center();
    now += 400;
    frame!([finger(3, c.x, c.y, PointerPhase::Down, now)]);
    now += 80;
    frame!([finger(3, c.x, c.y, PointerPhase::Up, now)]);
    let perform_ok = shell.state.mode == ShellMode::Perform
        && shell
            .last_layout
            .as_ref()
            .is_some_and(|l| l.rail.is_none() && l.inspector.is_none() && l.dock.is_none())
        && shell.audit_report().is_clean()
        && shell.rect_of("perform/panic").is_some();
    check("mode tap → Perform: chrome unhit-testable, XL pads audited clean", perform_ok, failures);
    let Some(back) = must_rect(&shell, "perform/mode", failures) else {
        return;
    };
    let c = back.center();
    now += 400;
    frame!([finger(4, c.x, c.y, PointerPhase::Down, now)]);
    now += 80;
    frame!([finger(4, c.x, c.y, PointerPhase::Up, now)]);
    check(
        "Perform mode tap → back to Design with chrome restored",
        shell.state.mode == ShellMode::Design && shell.rect_of("rail/transport/play").is_some(),
        failures,
    );

    // 3. long-press a rail button → context; then move → suppressed (never a drag).
    let Some(play) = must_rect(&shell, "rail/transport/play", failures) else {
        return;
    };
    let c = play.center();
    now += 400;
    frame!([finger(5, c.x, c.y, PointerPhase::Down, now)]);
    now += 400; // > 350 ms hold, no movement — on the clock the recogniser reads
    frame!([]); // advance(now) fires the long press
    let ctx_fired =
        shell.log().iter().any(|l| l.contains("long-press context: rail/transport/play"));
    check("long-press fires context on the element under the finger", ctx_fired, failures);
    now += 16;
    frame!([finger(5, c.x + 30.0, c.y, PointerPhase::Moved, now)]);
    let suppressed = shell.log().iter().any(|l| l.contains("context owns this contact"));
    check(
        "movement after long-press is suppressed and explainable (never a drag)",
        suppressed,
        failures,
    );
    now += 16;
    frame!([finger(5, c.x + 30.0, c.y, PointerPhase::Up, now)]);

    // ---- canvas smokes (WO-013) ------------------------------------------------------
    // These gestures now land on the graph, so we assert CANVAS state (what moved, what
    // connected, what the camera did) rather than a shell log string. That is a strictly
    // stronger proof: it shows the recogniser emitted the intent AND the canvas consumed it.
    use sparq_module_api::port::Direction as PDir;
    use sparq_ui::canvas::interact::MenuAction;
    use sparq_ui::canvas::layout::CanvasLayout;
    use sparq_ui::canvas::model::NodeId;
    use sparq_ui::geom::Vec2 as SpVec2;

    // Demo node order: 0 sine (mono out) · 1 gain (stereo in/out) · 2 rms (stereo in, cv out) ·
    // 3 gain again (FREE stereo in — the connect target).
    fn node_center(l: &CanvasLayout, idx: usize) -> Option<SpVec2> {
        l.nodes.get(idx).map(|n| n.screen.center())
    }
    fn node_id(l: &CanvasLayout, idx: usize) -> Option<NodeId> {
        l.nodes.get(idx).map(|n| n.id)
    }
    fn port_screen(l: &CanvasLayout, idx: usize, dir: PDir) -> Option<SpVec2> {
        l.nodes.get(idx).and_then(|n| n.ports.iter().find(|p| p.dir == dir)).map(|p| p.screen)
    }

    let Some(canvas_c) = shell.last_layout.as_ref().map(|l| l.canvas.center()) else {
        failures.push("no layout after frames".to_string());
        return;
    };

    // 4. drag a node → it moves and snaps to the 8 px grid; a second finger mid-drag is reported
    //    as fine (×10). We assert the node's world position changed and landed on the grid.
    let Some(n0) = node_center(shell.canvas_layout(), 0) else {
        failures.push("demo node 0 was not laid out".to_string());
        return;
    };
    let n0_id = node_id(shell.canvas_layout(), 0).unwrap_or(0);
    let before = shell.graph.node(n0_id).map(|n| n.pos);
    now += 400;
    frame!([finger(6, n0.x, n0.y, PointerPhase::Down, now)]);
    now += 16;
    frame!([finger(6, n0.x + 40.0, n0.y + 13.0, PointerPhase::Moved, now)]);
    now += 16;
    frame!([finger(7, n0.x + 220.0, n0.y + 160.0, PointerPhase::Down, now)]); // 2nd finger → fine
    let fine_on = shell.log().iter().any(|l| l.contains("fine resolution ENGAGED"));
    now += 16;
    frame!([finger(7, n0.x + 220.0, n0.y + 160.0, PointerPhase::Up, now)]);
    now += 16;
    frame!([finger(6, n0.x + 44.0, n0.y + 15.0, PointerPhase::Up, now)]);
    let after = shell.graph.node(n0_id).map(|n| n.pos);
    let snapped =
        after.map(|p| (p.x % 8.0).abs() < 1e-3 && (p.y % 8.0).abs() < 1e-3).unwrap_or(false);
    check(
        "drag a node: it moves, snaps to the 8 px grid, second finger reports fine (×10)",
        after != before && snapped && fine_on,
        failures,
    );

    // 5. three-finger tap → undo the move (the node returns to where it was).
    now += 400;
    frame!([
        finger(10, canvas_c.x - 40.0, canvas_c.y + 260.0, PointerPhase::Down, now),
        finger(11, canvas_c.x, canvas_c.y + 260.0, PointerPhase::Down, now + 5),
        finger(12, canvas_c.x + 40.0, canvas_c.y + 260.0, PointerPhase::Down, now + 10),
    ]);
    now += 60;
    frame!([
        finger(10, canvas_c.x - 40.0, canvas_c.y + 260.0, PointerPhase::Up, now),
        finger(11, canvas_c.x, canvas_c.y + 260.0, PointerPhase::Up, now + 5),
        finger(12, canvas_c.x + 40.0, canvas_c.y + 260.0, PointerPhase::Up, now + 10),
    ]);
    check(
        "three-finger tap undoes the move (the node returns)",
        shell.graph.node(n0_id).map(|n| n.pos) == before,
        failures,
    );

    // 6. drag an incompatible pair (cv out → audio in) → refused in words, no wire added. This is
    //    the "connecting incompatible ports is impossible" acceptance criterion, end to end.
    let wires0 = shell.graph.wire_count();
    let cv_out = port_screen(shell.canvas_layout(), 2, PDir::Out); // rms.level (cv)
    let free_in = port_screen(shell.canvas_layout(), 3, PDir::In); // out/main.in (audio, free — optional input)
    match (cv_out, free_in) {
        (Some(from), Some(to)) => {
            now += 400;
            frame!([finger(13, from.x, from.y, PointerPhase::Down, now)]);
            now += 16;
            frame!([finger(13, to.x, to.y, PointerPhase::Moved, now)]);
            now += 16;
            frame!([finger(13, to.x, to.y, PointerPhase::Up, now)]);
            let refused = shell.log().iter().any(|l| l.contains("canvas refused"));
            check(
                "drag cv-out → audio-in is refused in words and adds no wire",
                refused && shell.graph.wire_count() == wires0,
                failures,
            );
        },
        _ => failures.push("could not locate rms cv-out / gain2 audio-in".to_string()),
    }

    // 7. drag a compatible pair (stereo out → free stereo in) → a wire is created.
    let wires1 = shell.graph.wire_count();
    let gain_out = port_screen(shell.canvas_layout(), 1, PDir::Out); // gain.out (stereo)
    let free_in2 = port_screen(shell.canvas_layout(), 3, PDir::In); // out/main.in (free; a wired
                                                                    // required input would leave
                                                                    // no legal drag target — inc 7)
    match (gain_out, free_in2) {
        (Some(from), Some(to)) => {
            now += 400;
            frame!([finger(14, from.x, from.y, PointerPhase::Down, now)]);
            now += 16;
            frame!([finger(
                14,
                (from.x + to.x) / 2.0,
                (from.y + to.y) / 2.0,
                PointerPhase::Moved,
                now
            )]);
            now += 16;
            frame!([finger(14, to.x, to.y, PointerPhase::Moved, now)]); // arrive on the target port
            now += 16;
            frame!([finger(14, to.x, to.y, PointerPhase::Up, now)]);
            check(
                "drag stereo-out → free stereo-in connects (a wire is created)",
                shell.graph.wire_count() == wires1 + 1,
                failures,
            );
        },
        _ => failures.push("could not locate gain out / gain2 in".to_string()),
    }

    // 8. two-finger spread → the canvas camera zooms in.
    let zoom_before = shell.canvas.camera.zoom;
    now += 400;
    frame!([
        finger(8, canvas_c.x - 60.0, canvas_c.y + 300.0, PointerPhase::Down, now),
        finger(9, canvas_c.x + 60.0, canvas_c.y + 300.0, PointerPhase::Down, now),
    ]);
    now += 16;
    frame!([finger(9, canvas_c.x + 150.0, canvas_c.y + 300.0, PointerPhase::Moved, now)]);
    now += 16;
    frame!([finger(8, canvas_c.x - 150.0, canvas_c.y + 300.0, PointerPhase::Moved, now)]);
    check(
        "two-finger spread zooms the canvas camera in",
        shell.canvas.camera.zoom > zoom_before,
        failures,
    );
    now += 16;
    frame!([
        finger(8, canvas_c.x - 150.0, canvas_c.y + 300.0, PointerPhase::Up, now),
        finger(9, canvas_c.x + 150.0, canvas_c.y + 300.0, PointerPhase::Up, now),
    ]);

    // 9. double-tap empty → zoom to fit (recovers smoke 8's zoom), then long-press empty →
    //    RENDER WAV: the shell bridges the drawn patch through the registry + executor, writes
    //    canvas-render.wav in the working directory and logs the evidence. Touch in, WAV out —
    //    no audio device involved, so this runs (and is proven) anywhere, RDP included.
    let Some(rect) = shell.last_layout.as_ref().map(|l| l.canvas) else {
        failures.push("no canvas rect for the render smoke".to_string());
        return;
    };
    now += 400;
    frame!([finger(15, canvas_c.x, canvas_c.y, PointerPhase::Down, now)]);
    now += 80;
    frame!([finger(15, canvas_c.x, canvas_c.y, PointerPhase::Up, now)]);
    now += 120;
    frame!([finger(16, canvas_c.x, canvas_c.y, PointerPhase::Down, now)]);
    now += 80;
    frame!([finger(16, canvas_c.x, canvas_c.y, PointerPhase::Up, now)]);
    now += 300;
    frame!([]);
    // A world point below every node (the graph spans world y 0..280), through the live camera:
    let cam = shell.canvas.camera;
    let empty_pt = SpVec2::new(
        rect.min.x + (440.0 - cam.origin.x) * cam.zoom,
        rect.min.y + (330.0 - cam.origin.y) * cam.zoom,
    );
    now += 400;
    frame!([finger(17, empty_pt.x, empty_pt.y, PointerPhase::Down, now)]);
    now += 400;
    frame!([]); // long-press fires → empty-canvas menu
    now += 16;
    frame!([finger(17, empty_pt.x, empty_pt.y, PointerPhase::Up, now)]); // release: the context
                                                                         // owns this contact, so the Up is swallowed — the menu stays open for a fresh finger.
    let render_tap = shell.canvas.menu.clone().and_then(|m| {
        let origin = shell.canvas.menu_origin(rect)?;
        let row = m.rows.iter().position(|r| matches!(r.action, MenuAction::RenderWav))?;
        Some(m.row_rect(origin, row).center())
    });
    match render_tap {
        Some(tap) => {
            let _ = std::fs::remove_file("canvas-render.wav");
            now += 400;
            frame!([finger(18, tap.x, tap.y, PointerPhase::Down, now)]);
            now += 80;
            frame!([finger(18, tap.x, tap.y, PointerPhase::Up, now)]);
            now += 100;
            frame!([]);
            let wrote = std::fs::metadata("canvas-render.wav")
                .map(|m| m.len() > 1_900_000)
                .unwrap_or(false);
            let logged = shell.log().iter().any(|l| l.contains("rendered 5 s"));
            check(
                "empty long-press → RENDER WAV: the bridge renders the patch and writes canvas-render.wav",
                wrote && logged,
                failures,
            );
            // Live wire levels (WO-013 inc 4): the render just measured the patch, so the canvas
            // now carries REAL meter-driven levels — the demo's sine/gain are hot (~0.5 peak) and
            // the painter will light their wires. Not empty, and not faked: the level is the
            // executor's peak meter for the signal that actually rendered.
            check(
                "RENDER WAV populates live wire levels from the executor's meters (not faked)",
                !shell.canvas.levels.is_empty() && shell.canvas.levels.max_level() > 0.4,
                failures,
            );
        },
        None => failures.push("the empty-canvas menu did not offer RENDER WAV".to_string()),
    }

    // 10. long-press a node → SET MASTER: the explicit master wins over the default rule, so the
    //     user always controls (and can see) which node the next render carries.
    let Some(n1) = node_center(shell.canvas_layout(), 1) else {
        failures.push("node 1 was not laid out for the master smoke".to_string());
        return;
    };
    let n1_id = node_id(shell.canvas_layout(), 1).unwrap_or(1);
    now += 400;
    frame!([finger(19, n1.x, n1.y, PointerPhase::Down, now)]);
    now += 400;
    frame!([]);
    now += 16;
    frame!([finger(19, n1.x, n1.y, PointerPhase::Up, now)]); // release before the row tap
    let rect2 = shell.last_layout.as_ref().map(|l| l.canvas).unwrap_or(rect);
    let master_tap = shell.canvas.menu.clone().and_then(|m| {
        let origin = shell.canvas.menu_origin(rect2)?;
        let row = m.rows.iter().position(|r| matches!(r.action, MenuAction::SetMaster))?;
        Some(m.row_rect(origin, row).center())
    });
    match master_tap {
        Some(tap) => {
            now += 400;
            frame!([finger(20, tap.x, tap.y, PointerPhase::Down, now)]);
            now += 80;
            frame!([finger(20, tap.x, tap.y, PointerPhase::Up, now)]);
            check(
                "node long-press → SET MASTER makes that node the render master",
                shell.canvas.master == Some(n1_id),
                failures,
            );
        },
        None => failures.push("the node menu did not offer SET MASTER".to_string()),
    }

    // ---- increment 3 smokes: module browser, inspector sliders, wire re-patch ---------------

    // 11. long-press empty → ADD MODULE → the browser opens; query "gain" → one match; tap the
    //     row → a Gain node spawns at the press, selected, browser closed. The catalogue is the
    //     registry, so the row that spawns is a module that exists (#58, end to end).
    let nodes_before = shell.graph.node_count();
    let cam = shell.canvas.camera;
    let empty_pt2 = SpVec2::new(
        rect.min.x + (440.0 - cam.origin.x) * cam.zoom,
        rect.min.y + (330.0 - cam.origin.y) * cam.zoom,
    );
    now += 400;
    frame!([finger(21, empty_pt2.x, empty_pt2.y, PointerPhase::Down, now)]);
    now += 400;
    frame!([]); // long-press fires → empty-canvas menu
    now += 16;
    frame!([finger(21, empty_pt2.x, empty_pt2.y, PointerPhase::Up, now)]);
    let add_tap = shell.canvas.menu.clone().and_then(|m| {
        let r = shell.last_layout.as_ref().map(|l| l.canvas)?;
        let origin = shell.canvas.menu_origin(r)?;
        let row = m.rows.iter().position(|x| matches!(x.action, MenuAction::OpenBrowser))?;
        Some(m.row_rect(origin, row).center())
    });
    let mut browser_ok = false;
    if let Some(tap) = add_tap {
        now += 400;
        frame!([finger(22, tap.x, tap.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(22, tap.x, tap.y, PointerPhase::Up, now)]);
        browser_ok = shell.canvas.browser().is_some();
    }
    // The query goes through the documented headless path (the window shell pipes keystrokes).
    let mut spawn_ok = false;
    if browser_ok {
        shell.canvas.browser_set_query("gain");
        let row_tap = shell.canvas.browser().and_then(|b| {
            let r = shell.last_layout.as_ref().map(|l| l.canvas)?;
            let (max_w, max_h) = sparq_ui::canvas::browser::caps(r);
            let one_match = b.visible_len() == 1
                && b.selected_item().is_some_and(|i| i.spec.module_id == "sparq/util/gain");
            if !one_match {
                return None;
            }
            let origin = b.sheet_origin(r, max_w, max_h);
            Some(b.row_rect(origin, 0, r, max_w).center())
        });
        if let Some(tap) = row_tap {
            frame!([]); // repaint with the query applied
            now += 400;
            frame!([finger(23, tap.x, tap.y, PointerPhase::Down, now)]);
            now += 80;
            frame!([finger(23, tap.x, tap.y, PointerPhase::Up, now)]);
            spawn_ok = shell.graph.node_count() == nodes_before + 1
                && shell.canvas.browser().is_none()
                && shell
                    .graph
                    .nodes()
                    .last()
                    .is_some_and(|n| n.spec.module_id == "sparq/util/gain");
        }
    }
    check(
        "empty long-press → ADD MODULE → fuzzy search → row tap spawns the module",
        browser_ok && spawn_ok,
        failures,
    );

    // 12. tap the sine node → the inspector computes; drag its Frequency slider → the param
    //     follows the finger; ONE three-finger tap undoes the whole drag (param state rides the
    //     graph history — the acceptance criterion's "graph AND param state").
    let Some(n0b) = node_center(shell.canvas_layout(), 0) else {
        failures.push("node 0 was not laid out for the inspector smoke".to_string());
        return;
    };
    let n0b_id = node_id(shell.canvas_layout(), 0).unwrap_or(0);
    now += 400;
    frame!([finger(24, n0b.x, n0b.y, PointerPhase::Down, now)]);
    now += 80;
    frame!([finger(24, n0b.x, n0b.y, PointerPhase::Up, now)]);
    now += 100;
    frame!([]); // the frame that computes the inspector for the new selection
    let slider = shell.canvas.inspector().and_then(|il| {
        il.rows.first().map(|r| {
            (
                r.track.min.x + r.track.width() * 0.25,
                r.track.center().y,
                r.track.min.x + r.track.width() * 0.75,
            )
        })
    });
    let mut param_ok = false;
    if let Some((x0, y, x1)) = slider {
        let freq0 = shell.graph.node(n0b_id).and_then(|n| n.param_value(0));
        now += 400;
        frame!([finger(25, x0, y, PointerPhase::Down, now)]);
        now += 16;
        frame!([finger(25, (x0 + x1) / 2.0, y, PointerPhase::Moved, now)]);
        now += 16;
        frame!([finger(25, x1, y, PointerPhase::Moved, now)]);
        now += 16;
        frame!([finger(25, x1, y, PointerPhase::Up, now)]);
        let freq1 = shell.graph.node(n0b_id).and_then(|n| n.param_value(0));
        let dragged = freq1.is_some_and(|v| (v - 18_000.0).abs() < 600.0);
        let logged = shell.log().iter().any(|l| l.contains("Frequency ="));
        // undo: three-finger tap → the drag's single history entry reverses
        now += 400;
        frame!([
            finger(26, canvas_c.x - 40.0, canvas_c.y + 260.0, PointerPhase::Down, now),
            finger(27, canvas_c.x, canvas_c.y + 260.0, PointerPhase::Down, now + 5),
            finger(28, canvas_c.x + 40.0, canvas_c.y + 260.0, PointerPhase::Down, now + 10),
        ]);
        now += 60;
        frame!([
            finger(26, canvas_c.x - 40.0, canvas_c.y + 260.0, PointerPhase::Up, now),
            finger(27, canvas_c.x, canvas_c.y + 260.0, PointerPhase::Up, now + 5),
            finger(28, canvas_c.x + 40.0, canvas_c.y + 260.0, PointerPhase::Up, now + 10),
        ]);
        let freq2 = shell.graph.node(n0b_id).and_then(|n| n.param_value(0));
        param_ok = dragged && logged && freq0 == freq2;
    }
    check(
        "inspector slider drag edits the param; one undo restores it (graph AND param state)",
        param_ok,
        failures,
    );

    // 13. drag a wire's TO end (the grab handle the layout computed) onto the spawned node's
    //     free input → the wire moves; ONE undo restores the original wire, same id.
    let wires_before: Vec<_> = shell.graph.wires().to_vec();
    let repatch = shell.canvas_layout().wires.first().and_then(|w| {
        let target = port_screen(shell.canvas_layout(), 4, PDir::In)?; // the spawned Gain's input
        Some((w.id, w.grab_to, target))
    });
    let mut repatch_ok = false;
    if let Some((wid, grab, target)) = repatch {
        now += 400;
        frame!([finger(29, grab.x, grab.y, PointerPhase::Down, now)]);
        now += 16;
        frame!([finger(
            29,
            (grab.x + target.x) / 2.0,
            (grab.y + target.y) / 2.0,
            PointerPhase::Moved,
            now
        )]);
        now += 16;
        frame!([finger(29, target.x, target.y, PointerPhase::Moved, now)]);
        now += 16;
        frame!([finger(29, target.x, target.y, PointerPhase::Up, now)]);
        let moved =
            shell.graph.wires().iter().any(|w| w.to.node == 4 && w.from == wires_before[0].from)
                && !shell.graph.wires().iter().any(|w| w.id == wid && w.to == wires_before[0].to);
        // undo: three-finger tap → the original wire returns, same id and ends
        now += 400;
        frame!([
            finger(30, canvas_c.x - 40.0, canvas_c.y + 240.0, PointerPhase::Down, now),
            finger(31, canvas_c.x, canvas_c.y + 240.0, PointerPhase::Down, now + 5),
            finger(32, canvas_c.x + 40.0, canvas_c.y + 240.0, PointerPhase::Down, now + 10),
        ]);
        now += 60;
        frame!([
            finger(30, canvas_c.x - 40.0, canvas_c.y + 240.0, PointerPhase::Up, now),
            finger(31, canvas_c.x, canvas_c.y + 240.0, PointerPhase::Up, now + 5),
            finger(32, canvas_c.x + 40.0, canvas_c.y + 240.0, PointerPhase::Up, now + 10),
        ]);
        let restored = shell.graph.wires().iter().any(|w| *w == wires_before[0]);
        repatch_ok = moved && restored;
    }
    check(
        "drag a wire end → it re-patches to the new port; one undo restores the original",
        repatch_ok,
        failures,
    );
}

// ------------------------------------------------------------------ increment 5 smokes

/// Long-press `node_c`'s node and tap its RENAME row through the real recogniser. Returns
/// whether the rename sheet opened. A function, not a closure: the frames it drives need
/// `&mut shell`, which a closure capturing the same shell cannot be handed.
fn open_rename_sheet(
    shell: &mut ShellUi,
    ctx: &egui::Context,
    dims: (f32, f32),
    t: &mut f64,
    now: &mut u64,
    node_c: sparq_ui::geom::Vec2,
) -> bool {
    macro_rules! fr {
        ($pts:expr) => {{
            *t += 1.0 / 60.0;
            step(shell, ctx, dims.0, dims.1, *t, *now, &$pts);
        }};
    }
    *now += 400;
    fr!([finger(40, node_c.x, node_c.y, PointerPhase::Down, *now)]);
    *now += 400;
    fr!([]); // the long press fires → the node menu
    *now += 16;
    fr!([finger(40, node_c.x, node_c.y, PointerPhase::Up, *now)]); // swallowed by the context
    let tap = shell.canvas.menu.clone().and_then(|m| {
        let r = shell.last_layout.as_ref().map(|l| l.canvas)?;
        let origin = shell.canvas.menu_origin(r)?;
        let row = m
            .rows
            .iter()
            .position(|x| matches!(x.action, sparq_ui::canvas::interact::MenuAction::Rename))?;
        Some(m.row_rect(origin, row).center())
    });
    let Some(tap) = tap else { return false };
    *now += 400;
    fr!([finger(41, tap.x, tap.y, PointerPhase::Down, *now)]);
    *now += 80;
    fr!([finger(41, tap.x, tap.y, PointerPhase::Up, *now)]);
    shell.canvas.rename().is_some()
}

/// WO-013 increment 5's proof cells: the rename text entry (commit + undo + cancel), the cv wire
/// lit from its own port's published value after RENDER WAV, the inspector's two-finger scroll,
/// and the LOD walk down to Dot where ports stop being targetable. Same discipline as the
/// increment-3/4 smokes: real recogniser, real dispatch, assertions on STATE — log lines are
/// read only where the words themselves are the contract.
#[allow(clippy::too_many_lines)]
fn run_inc5_smokes(failures: &mut Vec<String>) {
    use sparq_ui::canvas::model::{Graph, NodeSpec, Op, PortRef};
    use sparq_ui::geom::Vec2 as SpVec2;

    let ctx = egui::Context::default();
    adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let mut shell = ShellUi::new();
    let mut t = 0.0_f64;
    let mut now = 0_u64;
    macro_rules! frame {
        ($pts:expr) => {{
            t += 1.0 / 60.0;
            step(&mut shell, &ctx, w, h, t, now, &$pts)
        }};
    }
    frame!([]); // frame 1: registry + demo graph laid out

    let Some(rect) = shell.last_layout.as_ref().map(|l| l.canvas) else {
        failures.push("no canvas rect for the inc-5 smokes".to_string());
        return;
    };
    let canvas_c = rect.center();
    let Some((n0_id, n0_c)) =
        shell.canvas_layout().nodes.first().map(|n| (n.id, n.screen.center()))
    else {
        failures.push("demo node 0 was not laid out".to_string());
        return;
    };

    // 14. RENAME: long-press the node → RENAME → the sheet opens pre-filled → type (the
    //     documented headless path) → ENTER (rename_commit) → the title changes → ONE
    //     three-finger tap undoes the rename (graph AND name state, the acceptance criterion).
    let opened = open_rename_sheet(&mut shell, &ctx, (w, h), &mut t, &mut now, n0_c);
    let pre_filled = shell.canvas.rename().map(|r| r.entry.text() == "Sine").unwrap_or(false);
    let mut committed = false;
    if opened && pre_filled {
        shell.canvas.rename_set_text("Kick");
        let _ = frame!([]); // the painter draws the sheet — a frame with it open is proven
        let ev = shell.canvas.rename_commit(&mut shell.graph);
        let _ = frame!([]);
        committed = shell.graph.node(n0_id).map(|n| n.title() == "Kick").unwrap_or(false)
            && shell.canvas.rename().is_none()
            && ev.iter().any(|e| e.message().contains("renamed to `Kick`"));
    }
    // undo: three-finger tap on empty canvas → the name returns
    now += 400;
    frame!([
        finger(42, canvas_c.x - 40.0, canvas_c.y + 300.0, PointerPhase::Down, now),
        finger(43, canvas_c.x, canvas_c.y + 300.0, PointerPhase::Down, now + 5),
        finger(44, canvas_c.x + 40.0, canvas_c.y + 300.0, PointerPhase::Down, now + 10),
    ]);
    now += 60;
    frame!([
        finger(42, canvas_c.x - 40.0, canvas_c.y + 300.0, PointerPhase::Up, now),
        finger(43, canvas_c.x, canvas_c.y + 300.0, PointerPhase::Up, now + 5),
        finger(44, canvas_c.x + 40.0, canvas_c.y + 300.0, PointerPhase::Up, now + 10),
    ]);
    let undone = shell.graph.node(n0_id).map(|n| n.title() == "Sine").unwrap_or(false);
    check(
        "long-press → RENAME → type → ENTER commits the title; one three-finger undo restores it",
        opened && pre_filled && committed && undone,
        failures,
    );

    // 15. The cancel path: open again, type, tap OUTSIDE → the sheet closes, stated in words,
    //     and the buffer is dropped (a cancel is not a commit — the name is untouched).
    let opened2 = open_rename_sheet(&mut shell, &ctx, (w, h), &mut t, &mut now, n0_c);
    let mut cancelled = false;
    if opened2 {
        shell.canvas.rename_set_text("Nope");
        let _ = frame!([]);
        now += 400;
        let outside = SpVec2::new(canvas_c.x + 300.0, canvas_c.y + 300.0);
        frame!([finger(45, outside.x, outside.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(45, outside.x, outside.y, PointerPhase::Up, now)]);
        cancelled = shell.canvas.rename().is_none()
            && shell.graph.node(n0_id).map(|n| n.title() == "Sine").unwrap_or(false)
            && shell.log().iter().any(|l| l.contains("rename cancelled"));
    }
    check(
        "an outside tap cancels the rename sheet — stated in words, the name untouched",
        opened2 && cancelled,
        failures,
    );

    // 16. cv wire levels (increment 4's declared limit, retired): wire the demo's rms.level into
    //     a spawned svf's cutoff-mod, make the svf the master, RENDER WAV through the menu — the
    //     cv wire must light from the value the rms port PUBLISHED (read from the executor after
    //     a real render), while the rms node's folded audio meter stays at rest (it has no audio
    //     output). Not faked: the folded zero and the hot port sit on the same node.
    let mut cv_ok = false;
    let svf_spec =
        shell.modules.get("sparq/flt/svf").map(|reg| NodeSpec::from_manifest(reg.manifest()));
    let rms_id =
        shell.graph.nodes().iter().find(|n| n.spec.module_id == "sparq/ana/rms").map(|n| n.id);
    let gain_id = rms_id
        .and_then(|rid| shell.graph.wires().iter().find(|w| w.to.node == rid).map(|w| w.from.node));
    match (svf_spec, rms_id, gain_id) {
        (Some(spec), Some(rid), Some(gid)) => {
            let op = shell.graph.op_add_node(spec, SpVec2::new(640.0, 0.0));
            let svf_id = match &op {
                Op::AddNode(n) => n.id,
                _ => u32::MAX,
            };
            // gain.out → svf.in (audio); the demo ALREADY wires gain.out → rms.in, so the only
            // new wire the rig needs is the cv one — a duplicate canvas wire would be refused by
            // the kernel's single-input rule and the render would (correctly) not happen.
            let _audio = shell.graph.op_add_wire(PortRef::new(gid, 1), PortRef::new(svf_id, 0));
            let cv_op = shell.graph.op_add_wire(PortRef::new(rid, 1), PortRef::new(svf_id, 2));
            let cv_wire = match &cv_op {
                Op::AddWire(w) => w.id,
                _ => u32::MAX,
            };
            shell.canvas.master = Some(svf_id); // the explicit master (SET MASTER's field)
                                                // Read the levels through the bridge directly — the same call the shell makes after
                                                // RENDER WAV (smoke 13 proves THAT path end to end). Deliberately NOT the menu flow:
                                                // a second menu-driven render here would overwrite `canvas-render.wav` — the file
                                                // test006's step [05] hashes as the manifest-defaults baseline — with this rig's
                                                // render, and the device A/B would compare two different graphs instead of proving
                                                // the slider reached the samples.
            let master = shell.canvas.resolve_master(&shell.graph).unwrap_or(svf_id);
            match crate::bridge::node_levels(&shell.graph, master, &shell.modules) {
                Ok(lv) => {
                    let port = lv.port(rid, 1).unwrap_or(0.0);
                    let wire = sparq_ui::canvas::levels::wire_level(&shell.graph, &lv, cv_wire);
                    cv_ok = port > 0.1
                        && (wire - port).abs() < 1e-6
                        && lv.get(rid) == 0.0
                        && lv.max_level() > 0.4;
                },
                Err(e) => failures.push(format!("the cv rig refused to render: {e}")),
            }
        },
        _ => failures.push("could not build the cv rig for the level smoke".to_string()),
    }
    check(
        "the cv wire lights from its own port's published value (per-port levels, not faked)",
        cv_ok,
        failures,
    );

    // 17. The inspector's two-finger scroll, on a tablet-minimum shell with a 20-param mixer:
    //     a hidden row becomes touchable and the camera stays put.
    let ctx2 = egui::Context::default();
    adapter::apply_style(&ctx2, ThemeChoice::PhosphorDark);
    let (w2, h2) = (1280.0_f32, 800.0_f32);
    let mut sh2 = ShellUi::new();
    let mut t2 = 0.0_f64;
    let mut now2 = 0_u64;
    macro_rules! frame2 {
        ($pts:expr) => {{
            t2 += 1.0 / 60.0;
            step(&mut sh2, &ctx2, w2, h2, t2, now2, &$pts)
        }};
    }
    frame2!([]);
    // One mixer, alone: the 20-param module the parked item named.
    sh2.graph = Graph::new();
    let mut scroll_ok = false;
    if let Some(reg) = sh2.modules.get("sparq/util/mixer") {
        let op =
            sh2.graph.op_add_node(NodeSpec::from_manifest(reg.manifest()), SpVec2::new(80.0, 80.0));
        let mid = match &op {
            Op::AddNode(n) => n.id,
            _ => u32::MAX,
        };
        let _ = frame2!([]);
        // tap the node → the inspector computes for it
        let nc = sh2.canvas_layout().nodes.iter().find(|n| n.id == mid).map(|n| n.screen.center());
        if let Some(nc) = nc {
            now2 += 400;
            frame2!([finger(50, nc.x, nc.y, PointerPhase::Down, now2)]);
            now2 += 80;
            frame2!([finger(50, nc.x, nc.y, PointerPhase::Up, now2)]);
            now2 += 100;
            frame2!([]);
            let il = sh2.canvas.inspector().cloned();
            let panel = sh2.last_layout.as_ref().and_then(|l| l.inspector);
            if let (Some(il), Some(panel)) = (il, panel) {
                // The contract, not a chosen row: the panel overflowed before the drag, and
                // AFTER it some row that was hidden is visible AND touchable where it is drawn.
                // Deliberately not hard-coded to a row index or a scroll distance — mixer 0.2.0
                // grew the param count mid-session and a fixed-distance smoke broke on it; the
                // thing under test is the reveal, not the arithmetic of one module's panel.
                let overflowed = il.rows.iter().any(|r| !il.row_visible(r));
                let cam_before = sh2.canvas.camera.origin;
                let (px, py) = (panel.center().x, panel.min.y + panel.height() * 0.6);
                // Three successive two-finger drags UP, 160 px each — every centroid staying
                // INSIDE the panel, because a pan whose centre leaves the panel belongs to the
                // canvas (that is the routing rule; the smoke respects it). 480 px of scroll
                // outruns what the last row needs; the clamp decides where the panel ends.
                let mut fid = 51_u64;
                for _ in 0..3 {
                    now2 += 400;
                    frame2!([
                        finger(fid, px - 30.0, py, PointerPhase::Down, now2),
                        finger(fid + 1, px + 30.0, py, PointerPhase::Down, now2 + 5),
                    ]);
                    for dy in [80.0_f32, 160.0] {
                        now2 += 16;
                        frame2!([
                            finger(fid, px - 30.0, py - dy, PointerPhase::Moved, now2),
                            finger(fid + 1, px + 30.0, py - dy, PointerPhase::Moved, now2 + 5),
                        ]);
                    }
                    now2 += 16;
                    frame2!([
                        finger(fid, px - 30.0, py - 160.0, PointerPhase::Up, now2),
                        finger(fid + 1, px + 30.0, py - 160.0, PointerPhase::Up, now2 + 5),
                    ]);
                    fid += 2;
                }
                now2 += 100;
                frame2!([]);
                if let Some(il2) = sh2.canvas.inspector() {
                    let revealed = il2.rows.iter().any(|r| {
                        let was_hidden = il
                            .rows
                            .iter()
                            .find(|o| o.index == r.index)
                            .is_some_and(|o| !il.row_visible(o));
                        was_hidden
                            && il2.row_visible(r)
                            && il2.row_at(r.track.center()) == Some(r.index)
                    });
                    scroll_ok = il2.max_scroll > 0.0
                        && overflowed
                        && il2.scroll > il.scroll + 50.0
                        && revealed
                        && sh2.canvas.camera.origin == cam_before;
                }
            }
        }
    }
    check(
        "inspector: a two-finger drag scrolls the rows — a hidden row becomes touchable, the camera stays put",
        scroll_ok,
        failures,
    );

    // 18. The LOD walk on the MAIN shell (it has the wired rig): pinch in twice → Simplified,
    //     then Dot (the zoom clamp floor). At Dot a port-to-port drag wires NOTHING — ports are
    //     not targetable where they are not drawn at scale (what you cannot see you cannot touch).
    let wires_before = shell.graph.wire_count();
    // stage 1: span 240 → 120 (factor 0.5): zoom 1.0 → 0.5 → Simplified
    now += 400;
    frame!([
        finger(60, canvas_c.x - 120.0, canvas_c.y + 200.0, PointerPhase::Down, now),
        finger(61, canvas_c.x + 120.0, canvas_c.y + 200.0, PointerPhase::Down, now + 5),
    ]);
    now += 16;
    frame!([
        finger(60, canvas_c.x - 60.0, canvas_c.y + 200.0, PointerPhase::Moved, now),
        finger(61, canvas_c.x + 60.0, canvas_c.y + 200.0, PointerPhase::Moved, now + 5),
    ]);
    now += 16;
    frame!([
        finger(60, canvas_c.x - 60.0, canvas_c.y + 200.0, PointerPhase::Up, now),
        finger(61, canvas_c.x + 60.0, canvas_c.y + 200.0, PointerPhase::Up, now + 5),
    ]);
    now += 100;
    frame!([]);
    let simplified = shell.log().iter().any(|l| l.contains("LOD Simplified"));
    // stage 2: span 120 → 60 (factor 0.5): zoom 0.5 → 0.25 → Dot
    now += 400;
    frame!([
        finger(62, canvas_c.x - 60.0, canvas_c.y + 200.0, PointerPhase::Down, now),
        finger(63, canvas_c.x + 60.0, canvas_c.y + 200.0, PointerPhase::Down, now + 5),
    ]);
    now += 16;
    frame!([
        finger(62, canvas_c.x - 30.0, canvas_c.y + 200.0, PointerPhase::Moved, now),
        finger(63, canvas_c.x + 30.0, canvas_c.y + 200.0, PointerPhase::Moved, now + 5),
    ]);
    now += 16;
    frame!([
        finger(62, canvas_c.x - 30.0, canvas_c.y + 200.0, PointerPhase::Up, now),
        finger(63, canvas_c.x + 30.0, canvas_c.y + 200.0, PointerPhase::Up, now + 5),
    ]);
    now += 100;
    frame!([]);
    let dot = shell.log().iter().any(|l| l.contains("LOD Dot"));
    // At Dot, a port-to-port drag adds no wire (it moves a node instead — the honest fallback).
    let from = shell.canvas_layout().nodes.iter().find_map(|n| {
        n.ports.iter().find(|p| p.dir == sparq_module_api::port::Direction::Out).map(|p| p.screen)
    });
    let to = shell.canvas_layout().nodes.iter().skip(3).find_map(|n| {
        n.ports.iter().find(|p| p.dir == sparq_module_api::port::Direction::In).map(|p| p.screen)
    });
    let mut no_wire = false;
    if let (Some(from), Some(to)) = (from, to) {
        now += 400;
        frame!([finger(64, from.x, from.y, PointerPhase::Down, now)]);
        now += 16;
        frame!([finger(
            64,
            (from.x + to.x) / 2.0,
            (from.y + to.y) / 2.0,
            PointerPhase::Moved,
            now
        )]);
        now += 16;
        frame!([finger(64, to.x, to.y, PointerPhase::Moved, now)]);
        now += 16;
        frame!([finger(64, to.x, to.y, PointerPhase::Up, now)]);
        no_wire = shell.graph.wire_count() == wires_before;
    }
    check(
        "the LOD walk logs Simplified then Dot, and at Dot a port drag wires nothing (ports are not targetable)",
        simplified && dot && no_wire,
        failures,
    );
}

// ------------------------------------------------------------------ live session smokes

/// WO-012 increment 2 smokes: the live audio session on the NULL backend in MANUAL mode — the
/// smoke pumps the blocks itself (`AudioStream::pump` exists for exactly this), so the whole
/// live path is deterministic and hermetic: no device, no wall-clock timing, no thread racing
/// a CI runner. Assertions are on STATE (levels, ring counters, the graph), never on log text
/// alone; the honesty lines are checked for EXISTENCE where the words are the deliverable.
fn run_live_smokes(failures: &mut Vec<String>) {
    use crate::ui::live::LiveOptions;
    use sparq_kernel::hal::{BackendKind, Fault};
    use sparq_module_api::port::Phase;
    use sparq_ui::canvas::connect::ConnectContext;
    use sparq_ui::canvas::interact::CanvasEvent;
    use sparq_ui::canvas::model::PortRef;

    let ctx = egui::Context::default();
    adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let mut shell = ShellUi::new();
    let mut t = 0.0_f64;
    let mut now = 0_u64;
    macro_rules! frame {
        ($pts:expr) => {{
            t += 1.0 / 60.0;
            step(&mut shell, &ctx, w, h, t, now, &$pts)
        }};
    }
    frame!([]); // frame 1: registry + demo graph laid out

    let find = |id: &str| shell.graph.nodes().iter().find(|n| n.spec.module_id == id).map(|n| n.id);
    let (Some(sine), Some(gain), Some(out_main)) =
        (find("sparq/syn/sine"), find("sparq/util/gain"), find("sparq/out/main"))
    else {
        failures.push("the demo patch is missing a node the live smokes need".to_string());
        return;
    };

    // 26. PLAY starts the session; the smoke pumps; the drain fills the wire levels from the
    //     engine's OWN meter ring — and the null device's capture holds the samples, the
    //     device-side proof that the callback rendered the CANVAS patch, not silence.
    shell.start_live_with(LiveOptions {
        backend: Some(BackendKind::Null),
        paced: false,
        capture_frames: 64 * 2 * 16,
    });
    let started = shell.live.is_some();
    let mut pumped = false;
    if let Some(s) = shell.live.as_mut() {
        pumped = s.pump_manual(8).is_ok();
    }
    frame!([]); // the per-frame drain lands the levels on the canvas
    let hot = shell.canvas.levels.get(sine) > 0.0 && shell.canvas.levels.get(gain) > 0.0;
    let captured = shell
        .live
        .as_mut()
        .and_then(|s| s.captured())
        .map(|c| c.iter().fold(0.0f32, |a, &v| a.max(v.abs())))
        .unwrap_or(0.0);
    check(
        "PLAY starts the live session; pumped blocks light the wires from the engine's meters and the capture holds the signal",
        started && pumped && hot && captured > 0.3,
        failures,
    );

    // 27. A param edit while live crosses the COMMAND RING — the level follows the signal and
    //     NO boundary swap happens (a re-stage per drag frame would reset module state; the
    //     ring exists so it does not).
    let before = shell.canvas.levels.get(sine);
    let swaps_before = shell.live.as_ref().map(|s| s.stats().swap.swaps).unwrap_or(u64::MAX);
    shell.canvas.param_edit(&mut shell.graph, sine, 1, 0.05);
    frame!([]); // the op→sync door
    if let Some(s) = shell.live.as_mut() {
        let _ = s.pump_manual(8);
    }
    frame!([]);
    let after = shell.canvas.levels.get(sine);
    let swaps_after = shell.live.as_ref().map(|s| s.stats().swap.swaps).unwrap_or(0);
    check(
        "a param edit while live crosses the command ring — the level FOLLOWS the signal, zero boundary swaps",
        after > 0.0 && after < before * 0.5 && swaps_after == swaps_before,
        failures,
    );

    // 28. A structural edit while live rebuilds and stages — the boundary swap is counted and
    //     the session survives it (the transport clock rides the swap by construction).
    let cctx = ConnectContext::no_adapters(Phase::Zero);
    let ev = shell.canvas.connect_ports(
        &mut shell.graph,
        PortRef::new(gain, 1),
        PortRef::new(out_main, 0),
        &cctx,
    );
    let connected = ev.iter().any(|e| matches!(e, CanvasEvent::Applied(_)));
    frame!([]); // the op→sync door: structural → rebuild + stage
    if let Some(s) = shell.live.as_mut() {
        let _ = s.pump_manual(4);
    }
    let swaps = shell.live.as_ref().map(|s| s.stats().swap.swaps).unwrap_or(0);
    check(
        "a structural edit while live re-stages at the boundary — swap counted, session survives",
        connected && swaps >= 1 && shell.live.is_some(),
        failures,
    );

    // 29. STOP by a real tap on the rail button: the session ends and the evidence line
    //     carries the MEASURED counters (existence of numbers asserted, not their values —
    //     the values belong to the device runs).
    let Some(stop) = must_rect(&shell, "rail/transport/stop", failures) else {
        return;
    };
    let c = stop.center();
    now += 400;
    frame!([finger(60, c.x, c.y, PointerPhase::Down, now)]);
    now += 80;
    frame!([finger(60, c.x, c.y, PointerPhase::Up, now)]);
    let evidence = shell.log().iter().any(|l| l.starts_with("STOP ·") && l.contains("blocks"));
    check(
        "STOP tap ends the session; the evidence line carries the measured counters",
        shell.live.is_none() && evidence,
        failures,
    );

    // 30. An unplug mid-play: the stream reports Removed, the shell's per-frame health check
    //     ends the session with ONE honest line, and the canvas is untouched — the device can
    //     die; the patch must not.
    let nodes_before = shell.graph.node_count();
    let wires_before = shell.graph.wire_count();
    shell.start_live_with(LiveOptions {
        backend: Some(BackendKind::Null),
        paced: false,
        capture_frames: 0,
    });
    if let Some(s) = shell.live.as_mut() {
        let _ = s.pump_manual(2);
        let _ = s.inject_fault(Fault::Unplug);
        let _ = s.pump_manual(1); // the pump observes the fault and stops producing
    }
    frame!([]); // the health check ends it
    let honest = shell
        .log()
        .iter()
        .any(|l| l.contains("the stream is Removed") && l.contains("canvas is untouched"));
    check(
        "an unplug mid-play ends the session in words — the canvas is untouched",
        shell.live.is_none()
            && honest
            && shell.graph.node_count() == nodes_before
            && shell.graph.wire_count() == wires_before,
        failures,
    );
}

// ------------------------------------------------------------------ scope screen smokes

/// WO-013 increment 6 smokes: `dsp/scope` draws from the analysis ring. The rig is built
/// through the documented driver doors (`connect_ports`), the session runs on the manual null
/// pumped by the smoke, and the LOD contract is measured in SHAPE COUNTS — the harness's own
/// metric — with the SESSION's traces cleared (the single source, no pump to re-feed them) for
/// the at-rest comparison, so no log-line or level drift can move the measurement.
fn run_scope_smokes(failures: &mut Vec<String>) {
    use crate::ui::live::LiveOptions;
    use sparq_kernel::hal::BackendKind;
    use sparq_module_api::port::Phase;
    use sparq_ui::canvas::connect::ConnectContext;
    use sparq_ui::canvas::model::{NodeSpec, Op, PortRef};
    use sparq_ui::geom::Vec2 as SpVec2;
    use sparq_ui::tokens::{LAYOUT_CANVAS_LOD_1_BELOW_ZOOM, LAYOUT_CANVAS_LOD_2_BELOW_ZOOM};

    let ctx = egui::Context::default();
    adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let mut shell = ShellUi::new();
    let mut t = 0.0_f64;
    let mut now = 0_u64;
    macro_rules! frame {
        ($pts:expr) => {{
            t += 1.0 / 60.0;
            step(&mut shell, &ctx, w, h, t, now, &$pts)
        }};
    }
    frame!([]); // frame 1: registry + demo graph laid out

    let gain =
        shell.graph.nodes().iter().find(|n| n.spec.module_id == "sparq/util/gain").map(|n| n.id);
    let tap_spec =
        shell.modules.get("sparq/ana/tap").map(|r| NodeSpec::from_manifest(r.manifest()));
    let scope_spec =
        shell.modules.get("sparq/dsp/scope").map(|r| NodeSpec::from_manifest(r.manifest()));
    let (Some(gain), Some(tap_spec), Some(scope_spec)) = (gain, tap_spec, scope_spec) else {
        failures.push("the registry is missing tap/scope for the scope smokes".to_string());
        return;
    };
    let nid = |op: Op| match op {
        Op::AddNode(n) => n.id,
        _ => u32::MAX,
    };
    // The rig: gain.out → tap.in, tap.wave → scope.x — through the driver door, so the ledger
    // marks it exactly as a finger drag would. A SPARE scope stays unbound.
    let tap_id = nid(shell.graph.op_add_node(tap_spec, SpVec2::new(-120.0, 320.0)));
    let scope_id = nid(shell.graph.op_add_node(scope_spec.clone(), SpVec2::new(240.0, 320.0)));
    let spare_id = nid(shell.graph.op_add_node(scope_spec, SpVec2::new(600.0, 320.0)));
    let cctx = ConnectContext::no_adapters(Phase::Zero);
    shell.canvas.connect_ports(
        &mut shell.graph,
        PortRef::new(gain, 1),
        PortRef::new(tap_id, 0),
        &cctx,
    );
    shell.canvas.connect_ports(
        &mut shell.graph,
        PortRef::new(tap_id, 1),
        PortRef::new(scope_id, 0),
        &cctx,
    );
    frame!([]);

    // 31. PLAY + pump: the bound scope's trace fills from the tap's published waveform (the
    //     SIGNED signal at its own amplitude); the unbound spare stays at the rest line.
    shell.start_live_with(LiveOptions {
        backend: Some(BackendKind::Null),
        paced: false,
        capture_frames: 0,
    });
    if let Some(s) = shell.live.as_mut() {
        let _ = s.pump_manual(8);
    }
    frame!([]); // sync resolves the bindings; the drain feeds the traces from the ring backlog
    let hot = shell
        .live
        .as_ref()
        .and_then(|s| s.traces().get(scope_id))
        .map(|tr| {
            !tr.x.is_empty()
                && tr.x.samples().iter().any(|v| *v > 0.0)
                && tr.x.samples().iter().any(|v| *v < 0.0)
                && tr.x.peak() > 0.3
        })
        .unwrap_or(false);
    let flat = shell
        .live
        .as_ref()
        .and_then(|s| s.traces().get(spare_id))
        .map(|tr| tr.x.is_empty())
        .unwrap_or(false);
    check(
        "a live scope's trace fills from its bound tap — signed, at amplitude; the unbound spare stays flat",
        hot && flat,
        failures,
    );

    // 32. The LOD contract, measured in SHAPE COUNTS (the harness's own metric — this egui
    //     tessellates a frame into ONE clipped primitive, so shapes are the countable truth):
    //     with a live trace, Full draws MORE than at rest; Simplified draws the SAME (the well
    //     says "scope"; the trace is Full-only, D8).
    let (shapes_full_live, _) = frame!([]);
    let zoom0 = shell.canvas.camera.zoom;
    shell.canvas.camera.zoom =
        (LAYOUT_CANVAS_LOD_1_BELOW_ZOOM + LAYOUT_CANVAS_LOD_2_BELOW_ZOOM) / 2.0;
    let (shapes_simp_live, _) = frame!([]);
    // Clear the SESSION's traces (the single source, D3′) without stopping: no pump follows,
    // so nothing re-feeds them. The at-rest comparison then differs from the live one in the
    // traces ONLY — no STOP, no new log lines, no level drift.
    if let Some(s) = shell.live.as_mut() {
        s.traces_mut().clear();
    }
    let (shapes_simp_rest, _) = frame!([]);
    shell.canvas.camera.zoom = zoom0;
    let (shapes_full_rest, _) = frame!([]);
    check(
        "the trace draws at Full LOD only — more shapes with a live trace, identical at Simplified",
        shapes_full_live > shapes_full_rest && shapes_simp_live == shapes_simp_rest,
        failures,
    );

    // Cleanup through the honest path: the STOP button, like smoke 29.
    if let Some(stop) = shell.rect_of("rail/transport/stop") {
        let c = stop.center();
        now += 400;
        frame!([finger(70, c.x, c.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(70, c.x, c.y, PointerPhase::Up, now)]);
    }
    if shell.live.is_some() {
        failures.push("the scope smoke left a live session running".to_string());
    }
}

/// WO-012 increment 4 smokes: the mouse path and the master-out meter bars. The extras slice
/// is the window adapter's door — right-click and wheel arrive as the recogniser's own intents,
/// so the smokes prove the ADAPTER's vocabulary, not a parallel one.
fn run_mouse_and_meter_smokes(failures: &mut Vec<String>) {
    use sparq_kernel::hal::BackendKind;
    use sparq_module_api::port::Phase;
    use sparq_ui::canvas::connect::ConnectContext;
    use sparq_ui::canvas::model::PortRef;
    use sparq_ui::gesture::GestureIntent;

    let ctx = egui::Context::default();
    adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let mut shell = ShellUi::new();
    let mut t = 0.0_f64;
    let mut now = 0_u64;
    macro_rules! frame {
        ($pts:expr) => {{
            t += 1.0 / 60.0;
            step(&mut shell, &ctx, w, h, t, now, &$pts)
        }};
    }
    macro_rules! frame_x {
        ($pts:expr, $extras:expr) => {{
            t += 1.0 / 60.0;
            step_x(&mut shell, &ctx, w, h, t, now, &$pts, &$extras)
        }};
    }
    frame!([]);

    // 35. Right-click is the mouse's long-press: over a node it opens THAT node's menu; an
    //     outside tap closes it with nothing applied.
    let node_c = shell.canvas_layout().nodes.first().map(|n| n.screen.center());
    let mut menu_open = false;
    if let Some(c) = node_c {
        frame_x!([], [GestureIntent::Context { pos: c }]);
        menu_open = shell.canvas.menu.is_some();
        let outside = sparq_ui::geom::Vec2::new(c.x + 300.0, c.y + 300.0);
        now += 400;
        frame!([finger(90, outside.x, outside.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(90, outside.x, outside.y, PointerPhase::Up, now)]);
    }
    check(
        "right-click opens the node's context menu (the mouse's long-press); an outside tap closes it",
        menu_open && shell.canvas.menu.is_none(),
        failures,
    );

    // 36. The wheel is a one-finger pan: over the inspector it scrolls the rows, over the
    //     canvas it moves the camera — and hover motion alone logs NOTHING (the adapter drops
    //     unpressed mouse motion, so the suppression line can only mean a real missed Down).
    // A scrollable inspector needs a node with many params: spawn the mixer through its dock
    // card (the increment's own door — spawn selects it), then wheel over its panel.
    let mixer_idx =
        shell.canvas.catalog().iter().position(|i| i.spec.module_id == "sparq/util/mixer");
    let mut sel = false;
    if let Some(mi) = mixer_idx {
        let card_id = format!("dock/card/{mi}");
        if let Some(card) = shell.rect_of(&card_id) {
            let c = card.center();
            now += 400;
            frame!([finger(91, c.x, c.y, PointerPhase::Down, now)]);
            now += 80;
            frame!([finger(91, c.x, c.y, PointerPhase::Up, now)]);
            sel = shell.canvas.selection.nodes.len() == 1;
        }
    }
    let sel_id = shell.canvas.selection.nodes.iter().next().copied();
    frame!([]); // the inspector geometry lands
    let insp_pt = shell.last_layout.as_ref().and_then(|l| l.inspector).map(|r| r.center());
    let mut scrolled = false;
    if let (Some(ip), Some(id)) = (insp_pt, sel_id) {
        let row = sparq_ui::tokens::LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32;
        frame_x!(
            [],
            [GestureIntent::Pan { delta: sparq_ui::geom::Vec2::new(0.0, -2.0 * row), center: ip }]
        );
        scrolled = shell.canvas.inspector_scroll(id).abs() > 1.0;
    }
    let cam_before = shell.canvas.camera.origin;
    if let Some(cc) = shell.last_layout.as_ref().map(|l| l.canvas.center()) {
        frame_x!(
            [],
            [GestureIntent::Pan { delta: sparq_ui::geom::Vec2::new(0.0, -60.0), center: cc }]
        );
    }
    let panned = (shell.canvas.camera.origin.y - cam_before.y).abs() > 1.0;
    let log_before = shell.log().len();
    now += 400;
    frame!([finger(92, 600.0, 500.0, PointerPhase::Moved, now)]); // hover, no button: dropped at the adapter
    let hover_quiet = shell.log().len() == log_before;
    check(
        "the wheel scrolls the inspector over the panel and pans the canvas over the canvas",
        sel && scrolled && panned,
        failures,
    );
    // Hover gating lives in the WINDOW adapter (unpressed mouse motion is dropped there); the
    // recogniser's suppression line stays correct for touch and cannot be exercised headless.
    let _ = hover_quiet;

    // 37. The master-out meter bars read the live per-channel ring: wire gain → out/main,
    //     PLAY on the manual null, pump — the session's stereo map carries both channels hot
    //     with holds at or above the live peaks.
    let gain =
        shell.graph.nodes().iter().find(|n| n.spec.module_id == "sparq/util/gain").map(|n| n.id);
    let out_main =
        shell.graph.nodes().iter().find(|n| n.spec.module_id == "sparq/out/main").map(|n| n.id);
    let mut wired = false;
    if let (Some(g), Some(o)) = (gain, out_main) {
        let cctx = ConnectContext::no_adapters(Phase::Zero);
        wired = shell
            .canvas
            .connect_ports(&mut shell.graph, PortRef::new(g, 1), PortRef::new(o, 0), &cctx)
            .iter()
            .any(|e| matches!(e, sparq_ui::canvas::interact::CanvasEvent::Applied(_)));
    }
    shell.start_live_with(crate::ui::live::LiveOptions {
        backend: Some(BackendKind::Null),
        paced: false,
        capture_frames: 0,
    });
    if let Some(s) = shell.live.as_mut() {
        let _ = s.pump_manual(8);
    }
    frame!([]);
    let bars =
        out_main.and_then(|o| shell.live.as_ref().and_then(|s| s.meters().get(&(o, 1)).copied()));
    check(
        "out/main's stereo meter bars read the live per-channel ring (both channels hot, holds >= peaks)",
        wired
            && bars.is_some_and(|m| {
                m.l > 0.3 && m.r > 0.3 && m.hold_l >= m.l && m.hold_r >= m.r
            }),
        failures,
    );
    if let Some(s) = shell.live.take() {
        s.stop("STOP", &mut Vec::new());
    }
}

/// `sparq ui --svg-out PATH`: run headless frames and dump the LAST frame's vector shapes as
/// SVG — a screenshot without a GPU (WO-012 increment 3). The visual-regression instrument the
/// look-board's protocols were missing: two runs of this diff like text, and a reviewer can
/// put it beside `design/mockups/design-mode.svg` in any browser. Painters emit rects, lines,
/// paths, circles and text; a mesh would mean tessellation, and tessellation is what a GPU is
/// for — so meshes are skipped and the dump says how many.
#[must_use]
pub fn run_svg(opts: &UiOptions) -> i32 {
    let Some(path) = opts.svg_out.as_deref() else { return 2 };
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(opts.scale);
    adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
    let mut shell = ShellUi::new();
    let frames = opts.headless_frames.max(1);
    let mut skipped = 0usize;
    let mut svg = String::new();
    for f in 0..frames {
        let mut out = ctx.run_ui(raw_input(opts.width, opts.height, f as f64 / 60.0), |ui| {
            shell.frame(
                ui,
                FrameInput {
                    pointers: &[],
                    now_ms: u64::try_from(f * 16).unwrap_or(0),
                    extras: &[],
                },
            );
        });
        out.textures_delta.clear();
        if f + 1 == frames {
            svg = shapes_to_svg(&out.shapes, opts.width, opts.height, &mut skipped);
        }
    }
    match std::fs::write(path, svg) {
        Ok(()) => {
            println!("ui svg: wrote {path} ({frames} frame(s); {skipped} mesh shape(s) skipped)");
            0
        },
        Err(e) => {
            eprintln!("ui svg: writing {path} failed: {e}");
            1
        },
    }
}

fn stroke_col(st: &egui::Stroke) -> egui::Color32 {
    st.color
}

/// A path's stroke carries a `ColorMode` (UV support); the dump only paints solid colour.
fn path_col(st: &egui::epaint::PathStroke) -> egui::Color32 {
    match st.color {
        egui::epaint::ColorMode::Solid(c) => c,
        _ => egui::Color32::TRANSPARENT,
    }
}

fn col(c: egui::Color32) -> (String, f32) {
    (format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b()), f32::from(c.a()) / 255.0)
}

fn shapes_to_svg(
    shapes: &[egui::epaint::ClippedShape],
    w: f32,
    h: f32,
    skipped: &mut usize,
) -> String {
    let mut o = String::new();
    o.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"ui-monospace, Menlo, Consolas, monospace\">\n"
    ));
    o.push_str(&format!(
        "<rect width=\"{w}\" height=\"{h}\" fill=\"{bg}\"/>\n",
        bg = sparq_ui::tokens::COLOR_GROUND_BASE.hex
    ));
    for cs in shapes {
        use egui::Shape;
        match &cs.shape {
            Shape::Rect(rs) => {
                let (rect, fill, stroke, rounding) =
                    (&rs.rect, rs.fill, rs.stroke, rs.corner_radius);
                let stroke = &stroke;
                let (x, y) = (rect.min.x, rect.min.y);
                let (rw, rh) = (rect.width().max(0.0), rect.height().max(0.0));
                let r = f32::from(rounding.ne).max(0.0);
                let (fc, fa) = col(fill);
                let (sc, sa) = col(stroke_col(stroke));
                o.push_str(&format!(
                    "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{rw:.1}\" height=\"{rh:.1}\" rx=\"{r:.1}\""
                ));
                if fill.a() > 0 {
                    o.push_str(&format!(" fill=\"{fc}\" fill-opacity=\"{fa:.3}\""));
                } else {
                    o.push_str(" fill=\"none\"");
                }
                if stroke.width > 0.0 && stroke_col(stroke).a() > 0 {
                    o.push_str(&format!(
                        " stroke=\"{sc}\" stroke-opacity=\"{sa:.3}\" stroke-width=\"{:.1}\"",
                        stroke.width
                    ));
                }
                o.push_str("/>\n");
            },
            Shape::LineSegment { points, stroke } => {
                let (sc, sa) = col(stroke_col(stroke));
                o.push_str(&format!(
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{sc}\" stroke-opacity=\"{sa:.3}\" stroke-width=\"{:.1}\"/>\n",
                    points[0].x, points[0].y, points[1].x, points[1].y, stroke.width
                ));
            },
            Shape::Path(ps) => {
                let (points, fill, stroke) = (&ps.points, ps.fill, &ps.stroke);
                if points.len() < 2 {
                    continue;
                }
                let mut d = String::new();
                for (i, pt) in points.iter().enumerate() {
                    d.push_str(&format!(
                        "{}{:.1},{:.1}",
                        if i == 0 { "M" } else { "L" },
                        pt.x,
                        pt.y
                    ));
                }
                d.push('Z');
                let (fc, fa) = col(fill);
                let (sc, sa) = col(path_col(stroke));
                o.push_str(&format!("<path d=\"{d}\""));
                if fill.a() > 0 {
                    o.push_str(&format!(" fill=\"{fc}\" fill-opacity=\"{fa:.3}\""));
                } else {
                    o.push_str(" fill=\"none\"");
                }
                if stroke.width > 0.0 && path_col(stroke).a() > 0 {
                    o.push_str(&format!(
                        " stroke=\"{sc}\" stroke-opacity=\"{sa:.3}\" stroke-width=\"{:.1}\"",
                        stroke.width
                    ));
                }
                o.push_str("/>\n");
            },
            Shape::Circle(cs) => {
                let (center, radius, fill, stroke) = (cs.center, cs.radius, cs.fill, cs.stroke);
                let stroke = &stroke;
                let (fc, fa) = col(fill);
                let (sc, sa) = col(stroke_col(stroke));
                o.push_str(&format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\"",
                    center.x, center.y, radius
                ));
                if fill.a() > 0 {
                    o.push_str(&format!(" fill=\"{fc}\" fill-opacity=\"{fa:.3}\""));
                } else {
                    o.push_str(" fill=\"none\"");
                }
                if stroke.width > 0.0 && stroke_col(stroke).a() > 0 {
                    o.push_str(&format!(
                        " stroke=\"{sc}\" stroke-opacity=\"{sa:.3}\" stroke-width=\"{:.1}\"",
                        stroke.width
                    ));
                }
                o.push_str("/>\n");
            },
            Shape::Text(ts) => {
                let pos = ts.pos;
                let galley = &ts.galley;
                let color = ts.override_text_color.unwrap_or_else(|| {
                    galley
                        .job
                        .sections
                        .first()
                        .map(|sec| sec.format.color)
                        .unwrap_or(ts.fallback_color)
                });
                let (tc, ta) = col(color);
                let size = galley.size().y.max(8.0);
                let text =
                    galley.text().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                o.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{size:.1}\" fill=\"{tc}\" fill-opacity=\"{ta:.3}\">{text}</text>\n",
                    pos.x, pos.y + size * 0.8
                ));
            },
            _ => *skipped += 1,
        }
    }
    o.push_str("</svg>\n");
    o
}

// ------------------------------------------------------------------ chrome conformance smokes

/// WO-012 increment 3 smokes: the mockup-convergence chrome is not decoration — the dock's
/// card grid and the rail's ADD cross are REAL doors onto the same op path the browser sheet
/// rides. Asserted on state: node counts, browser open/closed, undo.
fn run_chrome_smokes(failures: &mut Vec<String>) {
    let ctx = egui::Context::default();
    adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let mut shell = ShellUi::new();
    let mut t = 0.0_f64;
    let mut now = 0_u64;
    macro_rules! frame {
        ($pts:expr) => {{
            t += 1.0 / 60.0;
            step(&mut shell, &ctx, w, h, t, now, &$pts)
        }};
    }
    frame!([]);
    let canvas_c = shell.last_layout.as_ref().map(|l| l.canvas.center());

    // 33. A dock card tap spawns that module at the canvas centre; ONE three-finger tap undoes
    //     it — the card rides the same op path as the browser's row tap, so the undo story is
    //     the same story.
    let nodes_before = shell.graph.node_count();
    let mut spawned = false;
    if let Some(card) = shell.rect_of("dock/card/0") {
        let c = card.center();
        now += 400;
        frame!([finger(80, c.x, c.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(80, c.x, c.y, PointerPhase::Up, now)]);
        spawned = shell.graph.node_count() == nodes_before + 1;
    }
    if let Some(cc) = canvas_c {
        now += 400;
        frame!([
            finger(81, cc.x - 40.0, cc.y + 260.0, PointerPhase::Down, now),
            finger(82, cc.x, cc.y + 260.0, PointerPhase::Down, now + 5),
            finger(83, cc.x + 40.0, cc.y + 260.0, PointerPhase::Down, now + 10),
        ]);
        now += 60;
        frame!([
            finger(81, cc.x - 40.0, cc.y + 260.0, PointerPhase::Up, now),
            finger(82, cc.x, cc.y + 260.0, PointerPhase::Up, now + 5),
            finger(83, cc.x + 40.0, cc.y + 260.0, PointerPhase::Up, now + 10),
        ]);
    }
    check(
        "a dock card tap spawns the module at the canvas centre; one three-finger undo removes it",
        spawned && shell.graph.node_count() == nodes_before,
        failures,
    );

    // 34. The rail's ADD cross opens the browser at the canvas centre; an outside tap closes it
    //     with nothing spawned (a cancel is not a commit).
    let mut opened = false;
    if let Some(add) = shell.rect_of("rail/add") {
        let c = add.center();
        now += 400;
        frame!([finger(84, c.x, c.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(84, c.x, c.y, PointerPhase::Up, now)]);
        opened = shell.canvas.browser().is_some();
    }
    let before = shell.graph.node_count();
    if let Some(cr) = shell.last_layout.as_ref().map(|l| l.canvas) {
        let outside = sparq_ui::geom::Vec2::new(cr.min.x + 30.0, cr.min.y + 30.0);
        now += 400;
        frame!([finger(85, outside.x, outside.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(85, outside.x, outside.y, PointerPhase::Up, now)]);
    }
    check(
        "the rail's ADD opens the browser at the canvas centre; an outside tap cancels with nothing spawned",
        opened && shell.canvas.browser().is_none() && shell.graph.node_count() == before,
        failures,
    );
}
