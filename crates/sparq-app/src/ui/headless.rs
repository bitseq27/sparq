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
            "layout        : canvas {:.0}x{:.0} · rail {} · inspector {} · dock {} · small-viewport {} · forced {:?}",
            l.canvas.width(),
            l.canvas.height(),
            l.rail.is_some(),
            l.inspector.is_some(),
            l.dock.is_some(),
            l.below_breakpoint,
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
    println!("matrix: {} viewports x {} DPI scales x 1 mode (Design — Perform removed by operator ruling); every interactive element measured against its touch class", VIEWPORTS.len(), SCALES.len());

    // ------------------------------------------------------------ matrix
    // One mode now (Design — operator ruling 2026-09-30 removed Perform): the matrix is
    // viewports × DPI scales, and every scale must decide IDENTICALLY — the layout model works
    // in logical px, so DPI must not change what is decided or where it lands. That is the
    // machine-checkable half of the per-monitor-DPI acceptance criterion.
    for &(w, h, label) in &VIEWPORTS {
        let mut baseline: Option<(usize, usize, usize, String)> = None;
        let mut scale_results: Vec<String> = Vec::new();
        for &scale in &SCALES {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
            let mut shell = ShellUi::new();
            // Two frames: the first builds the registry, the second is what the audit reads.
            step(&mut shell, &ctx, w, h, 0.0, 0, &[]);
            step(&mut shell, &ctx, w, h, 1.0 / 60.0, 16, &[]);
            let report = shell.audit_report();
            let small = shell.last_layout.as_ref().is_some_and(|l| l.below_breakpoint);
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
                baseline = Some((key.0, key.1, key.2, lay.clone()));
            } else if let Some(b) = &baseline {
                if *b != (key.0, key.1, key.2, lay.clone()) {
                    failures.push(format!(
                        "{label} @{scale:.2}: DPI variance — {key:?} vs baseline {b:?}"
                    ));
                }
            }
            for v in &report.violations {
                failures.push(format!(
                    "{label} @{scale:.2}: {} needs {:.0}px, measured {:.0}px ({})",
                    v.id, v.required, v.actual, v.reason
                ));
            }
            scale_results.push(format!(
                "@{:.2}: {} elements, {} violations, {} dense{}",
                scale,
                report.checked,
                report.violations.len(),
                report.dense_badges.len(),
                if small { ", SMALL-REFLOW" } else { "" }
            ));
        }
        let verdict = if failures.is_empty() { "PASS" } else { "FAIL" };
        println!("[{verdict}] {label} · Design — {}", scale_results.join(" | "));
    }
    println!(
        "[{}] dpi-invariance — every viewport produced identical layout+audit at 100/125/150/200%",
        if failures.iter().any(|f| f.contains("DPI variance")) { "FAIL" } else { "PASS" }
    );

    // The below-breakpoint viewport must REFLOW in Design (the matrix cell above shows
    // SMALL-REFLOW; assert it explicitly so a regression cannot hide in a label): the shell
    // loads into Design, says the small-viewport sentence once, and collapses what the canvas
    // floor needs — never a refusal, never a second mode.
    {
        let ctx = egui::Context::default();
        adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
        let mut shell = ShellUi::new();
        step(&mut shell, &ctx, 1024.0, 700.0, 0.0, 0, &[]);
        let lay = shell.last_layout.clone();
        let ok = lay.as_ref().is_some_and(|l| {
            l.below_breakpoint
                && l.canvas.width() > 0.0
                && l.inspector.is_none()
                && l.forced.contains(&sparq_ui::shell::ForcedCollapse::Inspector)
        }) && shell.log().iter().any(|l| l.contains("tablet breakpoint"));
        if ok {
            println!(
                "[PASS] below-breakpoint — Design reflowed (inspector yielded), said in words"
            );
        } else {
            failures.push("below-breakpoint viewport did not reflow Design".to_string());
            println!("[FAIL] below-breakpoint — Design did not reflow");
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

    // ------------------------------------------------------------ round-3 smokes
    println!("round-3 smoke (r3: the junction bus collapses; control wires modulate):");
    run_r3_smokes(&mut failures);

    // ------------------------------------------------------------ response plot + inset wells
    println!(
        "response + inset smoke (WO-012 inc 5: wells on every audio node, the svf curve, the probe marker):"
    );
    run_response_smokes(&mut failures);

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

    // 2. The app LOADS into Design and there is no door out (operator ruling 2026-09-30:
    //    Perform mode removed from the runtime, Design is the one surface): no mode toggle in
    //    the top bar or the rail, no perform pads anywhere, and the Design chrome the rest of
    //    this suite drives is registered from the first frame.
    let no_mode_door = shell.rect_of("topbar/mode").is_none()
        && shell.rect_of("rail/mode/toggle").is_none()
        && shell.rect_of("perform/play").is_none()
        && shell.rect_of("perform/mode").is_none();
    check(
        "the shell loads straight into Design: no mode toggle, no perform pads, chrome registered",
        no_mode_door
            && shell.rect_of("rail/transport/play").is_some()
            && shell.rect_of("topbar/theme").is_some()
            && shell.audit_report().is_clean(),
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
    /// The grab point for BODY gestures (move, menu): the header band — since increment 6 the
    /// card centre is parameter rows, and a finger there edits, exactly like the reference.
    fn node_center(l: &CanvasLayout, idx: usize) -> Option<SpVec2> {
        l.nodes.get(idx).map(|n| n.header_screen.center())
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

    // 8. two-finger pinch zooms out and spread zooms in — under the 100 % ceiling (operator
    //    ruling 2026-10-01 r3): from home the spread has nowhere to go, so the smoke pinches
    //    first and watches the spread climb back to the ceiling, not past it.
    let zoom_home = shell.canvas.camera.zoom;
    now += 400;
    frame!([
        finger(8, canvas_c.x - 150.0, canvas_c.y + 300.0, PointerPhase::Down, now),
        finger(9, canvas_c.x + 150.0, canvas_c.y + 300.0, PointerPhase::Down, now),
    ]);
    now += 16;
    frame!([finger(9, canvas_c.x + 60.0, canvas_c.y + 300.0, PointerPhase::Moved, now)]);
    now += 16;
    frame!([finger(8, canvas_c.x - 60.0, canvas_c.y + 300.0, PointerPhase::Moved, now)]);
    let zoom_pinched = shell.canvas.camera.zoom;
    now += 16;
    frame!([finger(9, canvas_c.x + 150.0, canvas_c.y + 300.0, PointerPhase::Moved, now)]);
    now += 16;
    frame!([finger(8, canvas_c.x - 150.0, canvas_c.y + 300.0, PointerPhase::Moved, now)]);
    check(
        "two-finger pinch zooms out, spread zooms in, and 100 % is the ceiling",
        zoom_pinched < zoom_home - 1e-3
            && shell.canvas.camera.zoom > zoom_pinched + 1e-3
            && shell.canvas.camera.zoom <= zoom_home + 1e-4,
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
    let slider_track = shell.canvas.inspector().and_then(|il| il.rows.first().map(|r| r.track));
    let mut param_ok = false;
    if let (Some((x0, y, x1)), Some(track)) = (slider, slider_track) {
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
        // The expectation is the mapping's OWN answer at the release x (operator ruling
        // 2026-10-01: frequency rides the log map, 0.1 Hz – 10 kHz) — the smoke tests that
        // the param FOLLOWS THE FINGER through the one mapping, not the mapping's shape.
        let expected = shell
            .graph
            .node(n0b_id)
            .and_then(|n| n.spec.params.first().cloned())
            .map(|d| sparq_ui::canvas::inspector::value_from_x(&d, track, x1));
        let dragged = match (freq1, expected) {
            (Some(v), Some(e)) => (v - e).abs() <= (e * 0.02).max(1.0),
            _ => false,
        };
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
    // 1920: at tablet-min the reflow rule yields the INSPECTOR first (params live on the node
    // cards now; the library keeps the column) — this smoke is about the inspector's scroll,
    // so it runs where the inspector lives.
    let (w2, h2) = (1920.0_f32, 1080.0_f32);
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
        let nc = sh2
            .canvas_layout()
            .nodes
            .iter()
            .find(|n| n.id == mid)
            .map(|n| n.header_screen.center());
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
                // The coefficient GLIDES (defect #85: 25 ms tau), so the level follows with a fader's lag:
                // pump past two time constants before asking whether it arrived — the thing under test is
                // the ring path and the absence of swaps, not the glide's speed (param_ramp gates that).
    if let Some(s) = shell.live.as_mut() {
        let _ = s.pump_manual(48);
    }
    frame!([]);
    let after = shell.canvas.levels.get(sine);
    let swaps_after = shell.live.as_ref().map(|s| s.stats().swap.swaps).unwrap_or(0);
    check(
        "a param edit while live crosses the command ring — the level FOLLOWS the signal (through the glide), zero boundary swaps",
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
    let node_c = shell.canvas_layout().nodes.first().map(|n| n.header_screen.center());
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
        // The library's search is the door now: type the name, the ranking leaves one card.
        shell.library_entry = sparq_ui::canvas::entry::TextEntry::new("mixer");
        frame!([]);
        let card_id = format!("library/card/{mi}");
        if let Some(card) = shell.rect_of(&card_id) {
            let c = card.center();
            now += 400;
            frame!([finger(91, c.x, c.y, PointerPhase::Down, now)]);
            now += 80;
            frame!([finger(91, c.x, c.y, PointerPhase::Up, now)]);
            sel = shell.canvas.selection.nodes.len() == 1;
        }
        shell.library_entry = sparq_ui::canvas::entry::TextEntry::new("");
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
    // --review: the slice-B convergence state (WO-012 increment 5) — the wells' showcase
    // modules spawned beside the demo chain, the svf selected, and a pumped manual-null
    // session so the meter wells read LIVE (the rings' half of "live where they carry it").
    let frames =
        if opts.review { opts.headless_frames.max(4) } else { opts.headless_frames.max(1) };
    let mut review_svf: Option<sparq_ui::canvas::model::NodeId> = None;
    if opts.review {
        use sparq_ui::canvas::model::{NodeSpec, Op};
        let nid = |op: Op| match op {
            Op::AddNode(n) => n.id,
            _ => u32::MAX,
        };
        for (id, x, y) in [
            ("sparq/env/ad", 0.0f32, 400.0f32),
            ("sparq/mod/lfo", 0.0, 640.0),
            ("sparq/flt/svf", 800.0, 400.0),
            // A BARE required-input node on purpose: the review sheet shows the light-red
            // unfinished flag the operator's 2026-09-30 ruling introduced (flag, not refuse).
            ("sparq/util/delay", 1_360.0, 640.0),
        ] {
            if let Some(spec) = shell.modules.get(id).map(|r| NodeSpec::from_manifest(r.manifest()))
            {
                let spawned = nid(shell.graph.op_add_node(spec, sparq_ui::geom::Vec2::new(x, y)));
                if id == "sparq/flt/svf" {
                    review_svf = Some(spawned);
                }
            }
        }
        if let Some(id) = review_svf {
            // Wire the svf into the demo chain (gain.out → svf.in): a bare svf is an illegal
            // live patch (its audio input is required), and a WIRED one hands the master
            // badge over — the review sheet shows the filter rendering hot, meters live.
            if let Some(gain) = shell
                .graph
                .nodes()
                .iter()
                .find(|n| n.spec.module_id == "sparq/util/gain")
                .map(|n| n.id)
            {
                use sparq_module_api::port::Phase;
                use sparq_ui::canvas::connect::ConnectContext;
                use sparq_ui::canvas::model::PortRef;
                let cctx = ConnectContext::no_adapters(Phase::Zero);
                shell.canvas.connect_ports(
                    &mut shell.graph,
                    PortRef::new(gain, 1),
                    PortRef::new(id, 0),
                    &cctx,
                );
            }
            shell.canvas.selection.clear();
            shell.canvas.selection.nodes.insert(id);
        }
        // The scope rig joins the review sheet (operator ruling 2026-10-01 made the scope
        // screen bigger, with a graticule and measurements — the sheet is where a reviewer
        // sees it): tap reads the svf's output, the scope reads the tap's waveform — "the
        // wire is the binding, the ring is the payload".
        {
            use sparq_module_api::port::Phase;
            use sparq_ui::canvas::connect::ConnectContext;
            use sparq_ui::canvas::model::{NodeSpec, Op, PortRef};
            let nid = |op: Op| match op {
                Op::AddNode(n) => n.id,
                _ => u32::MAX,
            };
            let cctx = ConnectContext::no_adapters(Phase::Zero);
            let mut tap_id = None;
            let mut scope_id = None;
            for (id, x, y) in [
                ("sparq/ana/tap", 1_080.0f32, 160.0f32),
                ("sparq/dsp/scope", 1_080.0, 320.0),
                ("sparq/mod/clk", 1_360.0, 160.0),
                ("sparq/mod/seq", 1_360.0, 400.0),
            ] {
                if let Some(spec) =
                    shell.modules.get(id).map(|r| NodeSpec::from_manifest(r.manifest()))
                {
                    let spawned =
                        nid(shell.graph.op_add_node(spec, sparq_ui::geom::Vec2::new(x, y)));
                    if id == "sparq/ana/tap" {
                        tap_id = Some(spawned);
                    } else if id == "sparq/dsp/scope" {
                        scope_id = Some(spawned);
                    }
                }
            }
            if let (Some(svf), Some(tap)) = (review_svf, tap_id) {
                // svf.out (index 1) → tap.in (index 0)
                shell.canvas.connect_ports(
                    &mut shell.graph,
                    PortRef::new(svf, 1),
                    PortRef::new(tap, 0),
                    &cctx,
                );
            }
            if let (Some(tap), Some(scope)) = (tap_id, scope_id) {
                // tap.wave (index 1) → scope.x (index 0)
                shell.canvas.connect_ports(
                    &mut shell.graph,
                    PortRef::new(tap, 1),
                    PortRef::new(scope, 0),
                    &cctx,
                );
            }
        }
        // The clock family joins the sheet too (operator round 2): the clock's 16ths drive
        // the sequencer's ring — the event→event wire, visible and live.
        {
            use sparq_module_api::port::Phase;
            use sparq_ui::canvas::connect::ConnectContext;
            use sparq_ui::canvas::model::PortRef;
            let cctx = ConnectContext::no_adapters(Phase::Zero);
            let clk = shell
                .graph
                .nodes()
                .iter()
                .find(|n| n.spec.module_id == "sparq/mod/clk")
                .map(|n| n.id);
            let seq = shell
                .graph
                .nodes()
                .iter()
                .find(|n| n.spec.module_id == "sparq/mod/seq")
                .map(|n| n.id);
            if let (Some(clk), Some(seq)) = (clk, seq) {
                // clk.16ths (port index 2) → seq.clk (port 0)
                shell.canvas.connect_ports(
                    &mut shell.graph,
                    PortRef::new(clk, 2),
                    PortRef::new(seq, 0),
                    &cctx,
                );
            }
        }
        shell.start_live_with(crate::ui::live::LiveOptions {
            backend: Some(sparq_kernel::hal::BackendKind::Null),
            paced: false,
            capture_frames: 0,
        });
    }
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
        if opts.review && f == 0 {
            // After the first frame the inspector geometry (and with it the plot's axes) is
            // live: place the probe marker at the mockup's own frequency and pump the null
            // session so the next frames drain hot meters into the wells.
            if let Some(id) = review_svf {
                shell.canvas.set_response_marker(id, 1_240.0);
            }
            if let Some(s) = shell.live.as_mut() {
                let _ = s.pump_manual(16);
            }
        }
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
                // Only a CLOSED path gets the Z: an open stroke (a scope trace, a response
                // curve) must not grow a closing segment back to its first point — the well
                // would show a diagonal that was never drawn (instrument fix, WO-012 inc 5).
                if ps.closed {
                    d.push('Z');
                }
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

// ------------------------------------------------------------------ response plot + inset well smokes

/// One frame through the shell, keeping the two shape families the slice-B smokes assert on:
/// the drawn TEXTS (the honest-words half) and the open PATHS (the well/curve polyline half).
/// The pixel-side instrument for "the painter really drew it" — state assertions alone would
/// pass with a painter that never runs.
fn frame_collect(
    shell: &mut ShellUi,
    ctx: &egui::Context,
    w: f32,
    h: f32,
    t: f64,
    now: u64,
) -> (Vec<String>, Vec<egui::epaint::PathShape>, Vec<(egui::Rect, egui::Color32)>) {
    let mut out = ctx.run_ui(raw_input(w, h, t), |ui| {
        shell.frame(ui, FrameInput { pointers: &[], now_ms: now, extras: &[] });
    });
    out.textures_delta.clear();
    let mut texts = Vec::new();
    let mut paths = Vec::new();
    let mut fills = Vec::new();
    for cs in &out.shapes {
        match &cs.shape {
            egui::Shape::Text(ts) => texts.push(ts.galley.text().to_string()),
            egui::Shape::Path(ps) => paths.push(ps.clone()),
            egui::Shape::Rect(rs) if rs.fill.a() > 0 => fills.push((rs.rect, rs.fill)),
            _ => {},
        }
    }
    (texts, paths, fills)
}

/// Round-3 smokes: the junction bus collapses into direct kernel edges (the copy lights like
/// the original, the render runs), and a control wire from a cv source onto a float parameter
/// is audible in the rendered block (the modulation is not decoration).
fn run_r3_smokes(failures: &mut Vec<String>) {
    use sparq_module_api::port::Phase;
    use sparq_ui::canvas::connect::ConnectContext;
    use sparq_ui::canvas::model::PortRef;

    let ctx = egui::Context::default();
    adapter::apply_style(&ctx, ThemeChoice::PhosphorDark);
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let mut shell = ShellUi::new();
    let mut t = 0.0_f64;
    let now = 0_u64;
    macro_rules! frame {
        ($pts:expr) => {{
            t += 1.0 / 60.0;
            step(&mut shell, &ctx, w, h, t, now, &$pts)
        }};
    }
    frame!([]);

    // 51. sine → mult dot 0; mult dots 1 and 2 → two fresh gains: the bus collapses, the
    //     render runs, and every copy lights exactly like the source. (Fresh gains, because
    //     the demo's gain and rms inputs are single and already fed — a bus copy into an
    //     occupied single input is the fan-in replacement rule's business, not the bus's.)
    let cctx = ConnectContext::no_adapters(Phase::Zero);
    let mut bus_ok = false;
    let sine =
        shell.graph.nodes().iter().find(|n| n.spec.module_id == "sparq/syn/sine").map(|n| n.id);
    if let Some(sine) = sine {
        let modules = shell.modules.clone();
        let spawn = |shell: &mut ShellUi,
                     id: &str,
                     x: f32,
                     y: f32|
         -> Option<sparq_ui::canvas::model::NodeId> {
            let spec = modules
                .get(id)
                .map(|r| sparq_ui::canvas::model::NodeSpec::from_manifest(r.manifest()))?;
            match shell.graph.op_add_node(spec, sparq_ui::geom::Vec2::new(x, y)) {
                sparq_ui::canvas::model::Op::AddNode(n) => Some(n.id),
                _ => None,
            }
        };
        let mult = spawn(&mut shell, "sparq/util/mult", 400.0, 400.0);
        let g2 = spawn(&mut shell, "sparq/util/gain", 700.0, 300.0);
        let g3 = spawn(&mut shell, "sparq/util/gain", 700.0, 500.0);
        if let (Some(mid), Some(a), Some(b)) = (mult, g2, g3) {
            let w1 = shell.canvas.connect_ports(
                &mut shell.graph,
                PortRef::new(sine, 0),
                PortRef::new(mid, 0),
                &cctx,
            );
            let w2 = shell.canvas.connect_ports(
                &mut shell.graph,
                PortRef::new(mid, 1),
                PortRef::new(a, 0),
                &cctx,
            );
            let w3 = shell.canvas.connect_ports(
                &mut shell.graph,
                PortRef::new(mid, 2),
                PortRef::new(b, 0),
                &cctx,
            );
            let wired = [&w1, &w2, &w3].iter().all(|evs| {
                evs.iter().any(|e| matches!(e, sparq_ui::canvas::interact::CanvasEvent::Applied(_)))
            });
            let master = shell.canvas.resolve_master(&shell.graph).unwrap_or(a);
            let levels = crate::bridge::node_levels(&shell.graph, master, &shell.modules).ok();
            bus_ok = wired
                && levels.as_ref().is_some_and(|lv| {
                    let src = lv.get(sine);
                    src > 0.0
                        && (lv.get(mid) - src).abs() < 1e-6
                        && lv.get(a) > 0.0
                        && lv.get(b) > 0.0
                });
        }
    }
    check(
        "the junction bus collapses: one source, copied to two destinations, every copy lights like the original",
        bus_ok,
        failures,
    );

    // 52. A control wire is audible: lfo cv out → gain's Level parameter; the rendered peak
    //     moves off the plain knob value (the executor's param-mod plan, one block of latency).
    let mut mod_ok = false;
    let mut lfo =
        shell.graph.nodes().iter().find(|n| n.spec.module_id == "sparq/mod/lfo").map(|n| n.id);
    if lfo.is_none() {
        let modules = shell.modules.clone();
        if let Some(spec) = modules
            .get("sparq/mod/lfo")
            .map(|r| sparq_ui::canvas::model::NodeSpec::from_manifest(r.manifest()))
        {
            if let sparq_ui::canvas::model::Op::AddNode(n) =
                shell.graph.op_add_node(spec, sparq_ui::geom::Vec2::new(200.0, 700.0))
            {
                lfo = Some(n.id);
            }
        }
    }
    let gain =
        shell.graph.nodes().iter().find(|n| n.spec.module_id == "sparq/util/gain").map(|n| n.id);
    if let (Some(lfo), Some(gain)) = (lfo, gain) {
        // the lfo's cv OUT port, found by inspection (port 0 is its sync input)
        let cv_out = shell.graph.node(lfo).and_then(|n| {
            n.spec
                .ports
                .iter()
                .enumerate()
                .find(|(_, p)| {
                    p.direction == sparq_module_api::port::Direction::Out
                        && p.port_type == sparq_module_api::port::PortType::Cv
                })
                .map(|(i, _)| i)
        });
        let gidx = shell.graph.node(gain).and_then(|n| {
            n.spec
                .params
                .iter()
                .position(|d| d.kind == sparq_module_api::manifest::ParamKind::Float)
        });
        if let (Some(co), Some(gi)) = (cv_out, gidx) {
            let evs = shell.canvas.connect_param(&mut shell.graph, PortRef::new(lfo, co), gain, gi);
            mod_ok = evs
                .iter()
                .any(|e| matches!(e, sparq_ui::canvas::interact::CanvasEvent::Applied(_)));
        }
    }
    check(
        "a cv source onto a parameter's sink becomes a control wire (applied; the audible half lives in tests/r3_controls.rs)",
        mod_ok,
        failures,
    );
}

/// WO-012 increment 5 smokes (convergence slice B): the meter wells go live on EVERY
/// audio-output node (the ring already carried them), the inspector shows the svf curve from
/// the filter's own `magnitude_at`, the probe marker is a READING (dragging it moves the
/// readout and writes no param — the patch stays byte-identical), a no-curve module says so in
/// words, and the svf node thumbnail draws its curve.
fn run_response_smokes(failures: &mut Vec<String>) {
    use crate::ui::live::LiveOptions;
    use sparq_kernel::hal::BackendKind;
    use sparq_module_api::port::Phase;
    use sparq_ui::canvas::connect::ConnectContext;
    use sparq_ui::canvas::inset;
    use sparq_ui::canvas::model::{NodeSpec, Op, PortRef};
    use sparq_ui::canvas::response;
    use sparq_ui::geom::Vec2 as SpVec2;
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

    fn find_id(shell: &ShellUi, m: &str) -> Option<u32> {
        shell.graph.nodes().iter().find(|n| n.spec.module_id == m).map(|n| n.id)
    }
    let (gain, rms, sine) = (
        find_id(&shell, "sparq/util/gain"),
        find_id(&shell, "sparq/ana/rms"),
        find_id(&shell, "sparq/syn/sine"),
    );
    let nid = |op: Op| match op {
        Op::AddNode(n) => n.id,
        _ => u32::MAX,
    };

    // 38. Meter bars are `out/main`'s ALONE (operator ruling 2026-09-30): wire gain → out/main,
    //     PLAY + pump on the manual null, and the DATA-class bar fills appear inside out/main's
    //     body and inside NO other node's — the ring still carries every port's peaks (the
    //     session's map), but the painter spends them on the one meter the patch has. At rest
    //     the wells are empty.
    let rest_empty = shell.live.is_none();
    let out_main = find_id(&shell, "sparq/out/main");
    if let (Some(g), Some(o)) = (gain, out_main) {
        let cctx = ConnectContext::no_adapters(Phase::Zero);
        let _ = shell.canvas.connect_ports(
            &mut shell.graph,
            PortRef::new(g, 1),
            PortRef::new(o, 0),
            &cctx,
        );
    }
    shell.start_live_with(LiveOptions {
        backend: Some(BackendKind::Null),
        paced: false,
        capture_frames: 0,
    });
    if let Some(s) = shell.live.as_mut() {
        let _ = s.pump_manual(8);
    }
    let (_, _, fills) = {
        t += 1.0 / 60.0;
        frame_collect(&mut shell, &ctx, w, h, t, now)
    };
    let body_of =
        |id: u32| shell.canvas_layout().nodes.iter().find(|n| n.id == id).map(|n| n.screen);
    let data = adapter::Palette::for_theme(ThemeChoice::PhosphorDark).data;
    let bars_in = |id: Option<u32>| -> usize {
        body_of(id.unwrap_or(u32::MAX)).map_or(0, |b| {
            fills
                .iter()
                .filter(|(r, c)| {
                    *c == data
                        && r.min.x >= b.min.x - 1.0
                        && r.max.x <= b.max.x + 1.0
                        && r.min.y >= b.min.y - 1.0
                        && r.max.y <= b.max.y + 1.0
                })
                .count()
        })
    };
    let main_bars = bars_in(out_main);
    let other_bars = bars_in(gain) + bars_in(rms) + bars_in(sine);
    let main_hot = out_main
        .and_then(|o| shell.live.as_ref().and_then(|s| s.meters().get(&(o, 1)).copied()))
        .is_some_and(|m| m.l > 0.3 && m.r > 0.3 && m.hold_l >= m.l && m.hold_r >= m.r);
    check(
        "meter bars are out/main's alone (hot stereo wells there, none on any other node; empty at rest)",
        rest_empty && main_hot && main_bars >= 2 && other_bars == 0,
        failures,
    );
    if let Some(s) = shell.live.take() {
        s.stop("STOP", &mut Vec::new());
    }

    // The showcase modules for the wells: env/ad (Envelope), mod/lfo (Sparkline), flt/svf
    // (Curve) — spawned AFTER smoke 38's session on purpose: a BARE svf has an unwired
    // required audio input, which is an illegal live patch (WO-008 inc 7's host-side
    // enforcement would refuse the whole build — the session must not carry nodes the
    // executor refuses). Bare here changes no master: unwired nodes are excluded everywhere.
    let spawn = |shell: &mut ShellUi,
                 id: &str,
                 x: f32,
                 y: f32|
     -> Option<sparq_ui::canvas::model::NodeId> {
        let spec = shell.modules.get(id).map(|r| NodeSpec::from_manifest(r.manifest()))?;
        Some(nid(shell.graph.op_add_node(spec, SpVec2::new(x, y))))
    };
    let env_id = spawn(&mut shell, "sparq/env/ad", 100.0, 640.0);
    let lfo_id = spawn(&mut shell, "sparq/mod/lfo", 340.0, 640.0);
    let svf_id = spawn(&mut shell, "sparq/flt/svf", 580.0, 640.0);
    frame!([]);

    // 39. The inspector shows the svf curve (the filter's own math, on the fixed grid) and a
    //     tap on the plot places the probe marker: the readout answers in Hz and dB, and the
    //     marker's capture band joins the audit — measured, not hoped.
    let Some(svf) = svf_id else {
        failures.push("the registry is missing flt/svf for the response smokes".to_string());
        return;
    };
    shell.canvas.selection.clear();
    shell.canvas.selection.nodes.insert(svf);
    frame!([]); // the inspector geometry and the curve cache land
    let curve_ok = shell
        .curves
        .get(&svf)
        .map(|f| {
            f.freqs.len() == response::GRID_POINTS
                && f.mags.len() == response::GRID_POINTS
                && f.mags.iter().all(|m| m.is_finite() && *m >= 0.0)
                && f.mags[0] > f.mags[response::GRID_POINTS - 1] * 10.0
        })
        .unwrap_or(false);
    let plot = shell.canvas.inspector().and_then(|il| il.plot);
    let (mut readout_ok, mut audited) = (false, false);
    if let Some(plot) = plot {
        let before = shell.audit_report().checked;
        frame_x!([], [GestureIntent::Activate { pos: plot.center() }]);
        let report = shell.audit_report();
        audited = report.checked > before && report.violations.is_empty();
        readout_ok = shell
            .response_readout()
            .is_some_and(|(n, hz, db)| n == svf && hz > 100.0 && hz < 5_000.0 && db.is_finite());
    }
    check(
        "the inspector shows the svf curve and the tap-placed marker reads a frequency + dB (capture audited)",
        curve_ok && readout_ok && audited,
        failures,
    );

    // 40. Dragging the marker moves the readout and writes NO param: the patch is
    //     byte-identical after, the history is empty and the live-sync ledger heard nothing.
    let g0 = shell.graph.clone();
    let undo0 = shell.canvas.history.undo_len();
    let hz0 = shell.response_readout().map(|(_, hz, _)| hz);
    let mut moved = false;
    if let Some(plot) = shell.canvas.inspector().and_then(|il| il.plot) {
        let grab = SpVec2::new(plot.min.x + plot.width() * 0.75, plot.center().y);
        frame_x!([], [GestureIntent::DragStart { pos: grab }]);
        frame_x!([], [GestureIntent::DragUpdate { delta: SpVec2::new(-40.0, 0.0), scale: 1.0 }]);
        frame_x!(
            [],
            [GestureIntent::DragEnd { pos: SpVec2::new(grab.x - 40.0, grab.y), cancelled: false }]
        );
        moved = match (hz0, shell.response_readout().map(|(_, hz, _)| hz)) {
            (Some(a), Some(b)) => (b - a).abs() > 1.0,
            _ => false,
        };
    }
    let ledger_quiet = shell.canvas.take_patch_changes().is_empty();
    let idle = matches!(shell.canvas.interaction, sparq_ui::canvas::interact::Interaction::Idle);
    check(
        "dragging the marker moves the readout, writes no param and rings no command (the patch is byte-identical)",
        moved
            && shell.graph == g0
            && ledger_quiet
            && shell.canvas.history.undo_len() == undo0
            && idle,
        failures,
    );

    // 41. A module that declares no curve gets NO box and NO description (operator ruling
    //     2026-09-30 retired increment 5's no-curve words): the inspector shows the port strip
    //     and the rows, nothing about a curve is drawn or said, and the probe marker did not
    //     survive the selection change.
    let mut no_curve_clean = false;
    if let Some(sine_id) = sine {
        shell.canvas.selection.clear();
        shell.canvas.selection.nodes.insert(sine_id);
        t += 1.0 / 60.0;
        let (texts, _, _) = frame_collect(&mut shell, &ctx, w, h, t, now);
        let rows_under_title = shell.canvas.inspector().is_some_and(|il| {
            il.plot.is_none()
                && !il.rows.is_empty()
                && (il.rows[0].track.min.y - il.title.max.y).abs() < 1e-3
        });
        no_curve_clean = !shell.curves.contains_key(&sine_id)
            && shell.canvas.response_marker(sine_id).is_none()
            && rows_under_title
            && !texts.iter().any(|t| t.contains("NO RESPONSE CURVE"))
            && !texts.iter().any(|t| t == "RESPONSE");
    }
    check(
        "a no-curve module shows no response box and no description (rows start under the strip)",
        no_curve_clean,
        failures,
    );

    // 42. The node thumbnails draw: the svf well plots the response grid, the env/ad well its
    //     declared triangle, the lfo well its one-period outline — polylines INSIDE their node
    //     bodies, at the model's own point counts (the registry dispatched, the painter drew).
    let mut thumbs = (false, false, false);
    if let (Some(env), Some(lfo)) = (env_id, lfo_id) {
        t += 1.0 / 60.0;
        let (_, paths, _) = frame_collect(&mut shell, &ctx, w, h, t, now);
        let body_of = |id: sparq_ui::canvas::model::NodeId| {
            shell.canvas_layout().nodes.iter().find(|n| n.id == id).map(|n| n.screen)
        };
        let inside = |id: sparq_ui::canvas::model::NodeId, want: usize| -> bool {
            body_of(id).is_some_and(|body| {
                paths.iter().any(|ps| {
                    ps.points.len() == want
                        && ps.points.iter().all(|p| body.contains(SpVec2::new(p.x, p.y)))
                })
            })
        };
        thumbs = (
            inside(env, 2 * inset::ENVELOPE_POINTS),
            inside(lfo, inset::LFO_PERIOD_POINTS),
            inside(svf, response::GRID_POINTS),
        );
    }
    check(
        "the inset wells draw their param shapes (envelope triangle, lfo period, svf curve thumbnail)",
        thumbs.0 && thumbs.1 && thumbs.2,
        failures,
    );

    // 43. Playback is NOT blocked by a bare module (operator ruling 2026-09-30): the spawned
    //     svf still has no wire into its required `in`, and PLAY starts anyway — the executor
    //     flags it, the module renders silenced, and the canvas paints the light-red
    //     "unfinished" highlight (tint fill inside the body + the NO IN word at Full LOD).
    shell.start_live_with(LiveOptions {
        backend: Some(BackendKind::Null),
        paced: false,
        capture_frames: 0,
    });
    let playing =
        shell.live.as_ref().is_some_and(|s| s.health() == sparq_kernel::hal::StreamState::Running);
    let flagged = shell.graph.missing_required_inputs().contains(&(svf, 0));
    if let Some(s) = shell.live.as_mut() {
        let _ = s.pump_manual(4);
    }
    let pal = adapter::Palette::for_theme(ThemeChoice::PhosphorDark);
    let tint = egui::Color32::from_rgba_unmultiplied(
        pal.error.r(),
        pal.error.g(),
        pal.error.b(),
        (0.12f32 * 255.0).round() as u8,
    );
    let (texts43, _, fills43) = {
        t += 1.0 / 60.0;
        frame_collect(&mut shell, &ctx, w, h, t, now)
    };
    let svf_body = shell.canvas_layout().nodes.iter().find(|n| n.id == svf).map(|n| n.screen);
    let tinted = svf_body.is_some_and(|b| {
        fills43.iter().any(|(r, c)| {
            *c == tint
                && r.min.x >= b.min.x - 1.0
                && r.max.x <= b.max.x + 1.0
                && r.min.y >= b.min.y - 1.0
                && r.max.y <= b.max.y + 1.0
        })
    });
    check(
        "PLAY runs with a bare required input present: flagged (light-red tint + NO IN), silenced, not refused",
        playing && flagged && tinted && texts43.iter().any(|t| t == "NO IN"),
        failures,
    );
    if let Some(s) = shell.live.take() {
        s.stop("STOP", &mut Vec::new());
    }

    // 44. The log lives under the dock's LOG tab (operator ruling): with MODULES open no log
    //     line is on screen; a tap on LOG puts the tail there; a tap back clears it. The canvas
    //     band is gone.
    let last_line = shell.log().last().cloned().unwrap_or_default();
    // LOG is the dock's DEFAULT tab now (the palette moved to the library sidebar): with the
    // dock open the tail line is on screen, in the dock; collapse the dock and the line leaves
    // the frame entirely — the canvas band that used to carry it is gone for good.
    let (texts_open, _, _) = {
        t += 1.0 / 60.0;
        frame_collect(&mut shell, &ctx, w, h, t, now)
    };
    let on_log_tab = !last_line.is_empty()
        && shell.state.dock_tab == crate::ui::shell_ui::LOG_TAB
        && texts_open.contains(&last_line);
    let mut back_off = false;
    if let Some(dc) = shell.rect_of("dock/collapse") {
        let mc = dc.center();
        now += 400;
        frame!([finger(94, mc.x, mc.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(94, mc.x, mc.y, PointerPhase::Up, now)]);
        t += 1.0 / 60.0;
        let (texts_closed, _, _) = frame_collect(&mut shell, &ctx, w, h, t, now);
        back_off = shell.state.dock_collapsed && !texts_closed.contains(&last_line);
        now += 400;
        frame!([finger(95, mc.x, mc.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(95, mc.x, mc.y, PointerPhase::Up, now)]); // reopen for later smokes
    }
    check(
        "the log lives under the dock's LOG tab (on screen with the dock, gone with it, never on the canvas)",
        on_log_tab && back_off,
        failures,
    );

    // 45. The NODE LIBRARY is a real door: the search narrows with the browser's own ranking,
    //     the GROUP SWITCHES filter (operator ruling 2026-10-01: one toggle per module group),
    //     the wheel scrolls the card list where it overflows, and the count word says what the
    //     filter left — the sidebar cannot offer what the registry lacks (#58).
    use sparq_ui::canvas::entry::TextEntry;
    shell.library_entry = TextEntry::new("sine");
    let (texts45, _, _) = {
        t += 1.0 / 60.0;
        frame_collect(&mut shell, &ctx, w, h, t, now)
    };
    let sine_idx = shell.canvas.catalog().iter().position(|i| i.spec.module_id == "sparq/syn/sine");
    let n_sine = shell.library_items().len();
    let searched = sine_idx.is_some_and(|si| shell.library_items().first() == Some(&si))
        && n_sine < shell.canvas.catalog_len()
        && texts45.contains(&format!("{n_sine} NODES"));
    shell.library_entry = TextEntry::new("");
    frame!([]);
    let all = shell.library_items().len() == shell.canvas.catalog_len();
    // The group switches: flipping one OFF hides exactly that group; flipping it back ON
    // restores the full list. The first switch addresses the first category the registry
    // contains — the vocabulary is derived, never hard-coded.
    let mut toggled = false;
    let first_group = shell.library_categories().first().cloned();
    if let (Some(chip), Some(group)) = (shell.rect_of("library/group/0"), first_group) {
        let c = chip.center();
        now += 400;
        frame!([finger(96, c.x, c.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(96, c.x, c.y, PointerPhase::Up, now)]);
        toggled = shell.library_groups_off.contains(&group)
            && shell.library_items().len() < shell.canvas.catalog_len()
            && shell.library_items().iter().all(|&i| {
                shell.canvas.catalog()[i].spec.module_id.split('/').nth(1).unwrap_or("?") != group
            });
        now += 400;
        frame!([finger(97, c.x, c.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(97, c.x, c.y, PointerPhase::Up, now)]); // switch back ON
        toggled = toggled
            && shell.library_groups_off.is_empty()
            && shell.library_items().len() == shell.canvas.catalog_len();
    }
    shell.library_groups_off.clear(); // belt: every group shown again
    shell.library_scroll = 0.0;
    frame!([]);
    // The scroll: at 1080p the COMPACT tiles (operator ruling 2026-10-01) all fit, so there is
    // honestly nothing to scroll — the motion test runs at the tablet viewport, where the list
    // overflows and the pan must move it.
    let ctx2 = egui::Context::default();
    adapter::apply_style(&ctx2, ThemeChoice::PhosphorDark);
    let mut shell2 = ShellUi::new();
    let (w2, h2) = (1024.0_f32, 700.0_f32);
    step(&mut shell2, &ctx2, w2, h2, 0.0, 0, &[]);
    let scroll0 = shell2.library_scroll;
    if let Some(lib) = shell2.last_layout.as_ref().and_then(|l| l.library) {
        step_x(
            &mut shell2,
            &ctx2,
            w2,
            h2,
            1.0 / 60.0,
            16,
            &[],
            &[GestureIntent::Pan {
                delta: SpVec2::new(0.0, -200.0),
                center: SpVec2::new(lib.center().x, lib.center().y),
            }],
        );
    }
    let scrolled = shell2.library_scroll > scroll0 + 1.0;
    check(
        "the NODE LIBRARY searches, group-switches filter, the list scrolls where it overflows, and the count says it in words",
        searched && all && toggled && scrolled,
        failures,
    );
    shell.library_scroll = 0.0;
    frame!([]);

    // 46. An inline slider on a node card is the inspector's slider: drag edits the param
    //     (one undo step), and one three-finger undo restores the value the finger found.
    let gain_id = find_id(&shell, "sparq/util/gain");
    let row = shell
        .canvas_layout()
        .nodes
        .iter()
        .find(|n| n.id == gain_id.unwrap_or(u32::MAX))
        .and_then(|n| n.param_rows.first().copied());
    let (mut edited, mut undone) = (false, false);
    if let (Some(g), Some(row)) = (gain_id, row) {
        let before = shell.graph.node(g).and_then(|n| n.param_value(0));
        frame_x!([], [GestureIntent::DragStart { pos: row.track.center() }]);
        frame_x!([], [GestureIntent::DragUpdate { delta: SpVec2::new(40.0, 0.0), scale: 1.0 }]);
        frame_x!(
            [],
            [GestureIntent::DragEnd {
                pos: SpVec2::new(row.track.center().x + 40.0, row.track.center().y),
                cancelled: false
            }]
        );
        let after = shell.graph.node(g).and_then(|n| n.param_value(0));
        edited = before.is_some_and(|b| after.is_some_and(|a| (a - b).abs() > 1e-3));
        frame_x!([], [GestureIntent::Undo]);
        undone = shell.graph.node(g).and_then(|n| n.param_value(0)) == before;
    }
    check(
        "an inline node slider edits its param (one undo step) and one undo restores it",
        edited && undone,
        failures,
    );

    // 47. ARRANGE grid-arranges the patch as ONE undo step; one undo puts every node back.
    let pos0: Vec<(u32, SpVec2)> = shell.graph.nodes().iter().map(|n| (n.id, n.pos)).collect();
    if let Some(b) = shell.rect_of("toolbar/arrange") {
        let c = b.center();
        now += 400;
        frame!([finger(98, c.x, c.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(98, c.x, c.y, PointerPhase::Up, now)]);
    }
    let arranged = shell
        .graph
        .nodes()
        .iter()
        .any(|n| pos0.iter().find(|(id, _)| *id == n.id).is_some_and(|(_, p)| *p != n.pos));
    frame_x!([], [GestureIntent::Undo]);
    let pos1: Vec<(u32, SpVec2)> = shell.graph.nodes().iter().map(|n| (n.id, n.pos)).collect();
    check(
        "ARRANGE grids the patch in one undo step; one undo restores every position",
        arranged && pos1 == pos0,
        failures,
    );

    // 48. The zoom step buttons move the camera percent; RESET takes it home; and the
    //     ceiling is 100 % (operator ruling 2026-10-01 r3) — at home, zoom-in changes nothing.
    let tap_btn = |shell: &mut ShellUi, id: &str, now: &mut u64, fid: u64| {
        if let Some(b) = shell.rect_of(id) {
            let c = b.center();
            *now += 400;
            let _ = step(
                shell,
                &ctx,
                w,
                h,
                0.0,
                *now,
                &[finger(fid, c.x, c.y, PointerPhase::Down, *now)],
            );
            *now += 80;
            let _ = step(
                shell,
                &ctx,
                w,
                h,
                0.0,
                *now,
                &[finger(fid, c.x, c.y, PointerPhase::Up, *now)],
            );
        }
    };
    let z_home = shell.canvas.camera.zoom;
    tap_btn(&mut shell, "toolbar/zoom-in", &mut now, 99);
    let ceiling = (shell.canvas.camera.zoom - z_home).abs() < 1e-4 && z_home <= 1.0 + 1e-4;
    tap_btn(&mut shell, "toolbar/zoom-out", &mut now, 99);
    let z_out = shell.canvas.camera.zoom;
    let zout = z_out < z_home - 1e-3;
    tap_btn(&mut shell, "toolbar/zoom-in", &mut now, 99);
    let zin = shell.canvas.camera.zoom > z_out + 1e-3;
    tap_btn(&mut shell, "toolbar/reset", &mut now, 100);
    let home_again = (shell.canvas.camera.zoom - z_home).abs() < 1e-4;
    check(
        "the toolbar zoom steps move the camera percent, RESET takes it home, and 100 % is the ceiling",
        zin && zout && ceiling && home_again,
        failures,
    );

    // 49. The wire-style select is a display option with teeth: straight wires are two-point
    //     runs (grab points included), smooth wires are sampled beziers — and the hit-test
    //     restyles with the painter, so what you see is what you can grab.
    let straight_before =
        shell.canvas_layout().wires.iter().filter(|w| w.points.len() == 2).count();
    if let Some(b) = shell.rect_of("toolbar/wire-style") {
        let c = b.center();
        now += 400;
        frame!([finger(101, c.x, c.y, PointerPhase::Down, now)]);
        now += 80;
        frame!([finger(101, c.x, c.y, PointerPhase::Up, now)]);
    }
    let straight_after = shell.canvas_layout().wires.iter().filter(|w| w.points.len() == 2).count();
    let wires = shell.graph.wire_count();
    check(
        "the wire-style select restyles painter AND hit-test (beziers <-> straight runs)",
        wires > 0 && straight_before == 0 && straight_after == wires,
        failures,
    );

    // 50. Main Out is the driver's window (operator ruling 2026-10-01): the right-edge IN/OUT
    //     strip is GONE from the workspace, and the permanent out/main card says which driver
    //     is under it — the at-rest words before PLAY, the negotiated truth (backend · device ·
    //     rate · channels · block · format) while the session runs.
    let Some(canvas_r) = shell.last_layout.as_ref().map(|l| l.canvas) else {
        failures.push("no canvas rect for the main-out smoke".to_string());
        return;
    };
    let strip = sparq_ui::geom::Rect::new(
        SpVec2::new(canvas_r.max.x - 40.0, canvas_r.min.y),
        SpVec2::new(canvas_r.max.x - 8.0, canvas_r.max.y),
    );
    let (texts_rest, _, fills_rest) = {
        t += 1.0 / 60.0;
        frame_collect(&mut shell, &ctx, w, h, t, now)
    };
    let strip_fills = |fills: &Vec<(egui::Rect, egui::Color32)>| {
        fills
            .iter()
            .filter(|(r, c)| {
                *c == pal.data
                    && r.min.x >= strip.min.x - 1.0
                    && r.max.x <= strip.max.x + 1.0
                    && r.height() > 1.0
            })
            .count()
    };
    let rest_ok =
        texts_rest.iter().any(|s| s.contains("NO SESSION")) && strip_fills(&fills_rest) == 0;
    shell.start_live_with(LiveOptions {
        backend: Some(BackendKind::Null),
        paced: false,
        capture_frames: 0,
    });
    if let Some(ss) = shell.live.as_mut() {
        let _ = ss.pump_manual(8);
    }
    let driver = shell.live.as_ref().map(|ss| ss.driver_lines());
    let (texts_hot, _, _) = {
        t += 1.0 / 60.0;
        frame_collect(&mut shell, &ctx, w, h, t, now)
    };
    let live_ok = driver.as_ref().is_some_and(|lines| {
        let head: String = lines[0].chars().take(12).collect();
        texts_hot.iter().any(|s| s == &lines[1])
            && texts_hot.iter().any(|s| s.starts_with(&head))
            && lines[1].contains("kHz")
            && lines[1].contains("ch")
            && lines[1].contains("fr")
            && lines[1].contains("f32")
    });
    check(
        "the right-edge IN/OUT strip is gone; the Main Out card reads the driver at rest and the negotiated truth while playing",
        rest_ok && live_ok,
        failures,
    );
    if let Some(ss) = shell.live.take() {
        ss.stop("STOP", &mut Vec::new());
    }
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
    if let Some(card) = shell.rect_of("library/card/0") {
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
        // Below the toolbar band (chrome since increment 6) and clear of every node: an
        // outside tap is a tap the canvas owns.
        let outside = sparq_ui::geom::Vec2::new(
            cr.min.x + 30.0,
            cr.min.y + sparq_ui::tokens::LAYOUT_SHELL_TOOLBAR_HEIGHT as f32 + 30.0,
        );
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
