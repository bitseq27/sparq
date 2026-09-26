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
    let mut out = ctx.run_ui(raw_input(w, h, t_secs), |ui| {
        shell.frame(ui, FrameInput { pointers, now_ms });
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
    let free_in = port_screen(shell.canvas_layout(), 3, PDir::In); // gain2.in (audio, free)
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
    let free_in2 = port_screen(shell.canvas_layout(), 3, PDir::In); // gain2.in (free)
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
