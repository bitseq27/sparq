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

    // ------------------------------------------------------------ increment 5 smoke
    println!("increment 5 smoke (rename entry, cv wire levels, inspector scroll, the LOD walk):");
    run_inc5_smokes(&mut failures);

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
