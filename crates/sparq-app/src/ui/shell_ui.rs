//! The shell, drawn (WO-012 task 3): rail / top bar / canvas / inspector / dock from
//! `sparq_ui::shell::compute`, styled exclusively through the token adapter, with every
//! interactive element registered for the layout audit.
//!
//! Architecture note — the WO-012 risk column says: *"wrap egui: all widgets go through your own
//! gesture layer so the Phase 6 swap only replaces the drawing half."* That is exactly what this
//! file is. There are **no egui buttons in here**: egui is used as a painter (rects, lines,
//! text), while hit-testing runs on sparq's own [`GestureRecognizer`] against rects sparq
//! computed. If egui were replaced tomorrow, the `draw_*` bodies change and nothing else does —
//! input, layout, audit and state all live in the zero-dependency crate.
//!
//! No colour, size or spacing literal may appear in this file (acceptance criterion: "no
//! hard-coded colour/size/spacing values in widget code"; `token_audit.py` R6 enforces).
//! Everything is a token constant or a `Palette` field.

use egui::{Align2, Color32, Painter, Stroke};
use sparq_module_api::port::Phase;
use sparq_module_api::registry::Registry;
use sparq_ui::audit::{InteractiveElement, TouchClass};
use sparq_ui::canvas::connect::ConnectContext;
use sparq_ui::canvas::interact::{CanvasEvent, CanvasState, Interaction};
use sparq_ui::canvas::layout::CanvasLayout;
use sparq_ui::canvas::model::Graph;
use sparq_ui::geom::{Rect, Vec2};
use sparq_ui::gesture::{Edge, GestureIntent, GestureRecognizer};
use sparq_ui::pointer::PointerEvent;
use sparq_ui::shell::{self, ShellLayout, ShellMode, ShellState};
use sparq_ui::tokens::*;

use crate::ui::adapter::{
    egui_rect, font_l, font_m, font_s, font_xs, sp_rect, Palette, ThemeChoice,
};
use crate::ui::canvas_ui;

/// How many intent-log lines the canvas shows (a count, not a visual size).
const LOG_LINES: usize = 16;

/// What a registered interactive element *does* when activated. The recogniser decides THAT a
/// tap happened and WHERE; this decides what it means. Adding behaviour never touches the
/// gesture layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Flip Design <-> Perform.
    ToggleMode,
    /// Flip phosphor-dark <-> contrast-high.
    ToggleTheme,
    /// Collapse/expand the rail.
    ToggleRail,
    /// Collapse/expand the inspector.
    ToggleInspector,
    /// Collapse/expand the dock.
    ToggleDock,
    /// Transport: play (a real binding lands with WO-007/008; the shell logs it).
    Play,
    /// Transport: stop.
    Stop,
    /// All sound off. The only button that is red before you press it.
    Panic,
}

/// Everything needed to draw + register one control, as a value: `button()` stays readable and
/// call sites declare controls as data, which is what a control IS at this layer.
struct Button {
    id: &'static str,
    rect: Rect,
    label: &'static str,
    class: TouchClass,
    dense_allowed: bool,
    action: Action,
    accent: Option<Color32>,
}

impl Button {
    fn new(
        id: &'static str,
        rect: Rect,
        label: &'static str,
        class: TouchClass,
        action: Action,
    ) -> Self {
        Self { id, rect, label, class, dense_allowed: false, action, accent: None }
    }
    fn dense(mut self) -> Self {
        self.dense_allowed = true;
        self
    }
    fn accent(mut self, c: Color32) -> Self {
        self.accent = Some(c);
        self
    }
}

/// One interactive element as laid out this frame: audit data + where it is + what it does.
struct Registered {
    id: &'static str,
    rect: Rect,
    #[allow(dead_code)]
    // the class travels with the element for diagnostics; the audit reads its own copy
    class: TouchClass,
    action: Action,
}

/// Everything the shell carries between frames.
pub struct ShellUi {
    /// Panel collapse flags, mode, user-resized dimensions.
    pub state: ShellState,
    /// Active colour theme.
    pub theme: ThemeChoice,
    /// The toolkit-independent recogniser (input-model §3).
    pub recognizer: GestureRecognizer,
    /// Last frame's interactive registry — hit-testing an activation uses the frame the user
    /// actually saw, which in immediate mode is the previous one.
    registry: Vec<Registered>,
    /// This frame's audit elements (rebuilt every frame; `audit_report()` reads them).
    audit_elements: Vec<InteractiveElement>,
    /// Intent/diagnostic log shown on the canvas — the shell explains itself.
    log: Vec<String>,
    /// Last computed layout (diagnostics, headless assertions).
    pub last_layout: Option<ShellLayout>,
    /// Set while the viewport is below the tablet breakpoint: Design is refused and Perform is
    /// forced (§14.2 breakpoint rules). Rendered as a banner — refusals are stated in words.
    design_refused: bool,
    /// The theme changed this frame; `frame()` re-runs the style adapter on the real context.
    theme_dirty: bool,
    /// The patch graph being edited (WO-013). The demo set is built from the registry — the same
    /// manifests discovery reads; the browser lands with the next increment.
    pub graph: Graph,
    /// The first-party module registry (WO-014). The canvas→executor bridge instantiates from it;
    /// the canvas never shows a module the registry does not have (defect #58).
    pub modules: Registry,
    /// Canvas camera, selection, in-flight interaction, context menu and undo history (WO-013).
    pub canvas: CanvasState,
    /// Last frame's computed canvas layout — hit-testing an activation uses the frame the user
    /// actually saw, exactly like the shell's `registry`.
    canvas_layout: CanvasLayout,
}

/// Per-frame inputs the host (window or headless driver) supplies.
pub struct FrameInput<'a> {
    /// Normalised pointer events since the last frame (winit adapter or synthetic).
    pub pointers: &'a [PointerEvent],
    /// Monotonic milliseconds — the recogniser never reads a clock itself.
    pub now_ms: u64,
}

impl Default for ShellUi {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellUi {
    /// A fresh shell in Design mode with the default theme.
    #[must_use]
    pub fn new() -> Self {
        let mut modules = Registry::new();
        let mut log: Vec<String> = Vec::new();
        if let Err(e) = sparq_audio::modules::register_builtins(&mut modules) {
            log.push(format!("built-in registration FAILED: {e}"));
        }
        let graph = match crate::bridge::demo_graph(&modules) {
            Ok(g) => g,
            Err(e) => {
                log.push(format!("demo patch REFUSED: {e}"));
                Graph::new()
            },
        };
        // The browser's catalogue is the registry, viewed (increment 3): built once at startup,
        // from the validated manifests — the browser cannot offer what is not installed (#58).
        let mut canvas = CanvasState::new();
        canvas.set_catalog(crate::bridge::browser_catalog(&modules));
        Self {
            state: ShellState::default(),
            theme: ThemeChoice::PhosphorDark,
            recognizer: GestureRecognizer::from_tokens(),
            registry: Vec::new(),
            audit_elements: Vec::new(),
            log,
            last_layout: None,
            design_refused: false,
            theme_dirty: false,
            graph,
            modules,
            canvas,
            canvas_layout: CanvasLayout::default(),
        }
    }

    /// One frame: consume pointers → recognise gestures → apply intents → compute layout →
    /// draw → rebuild the registry and the audit list, in that order. Called by both the winit
    /// loop and the headless driver — the two hosts differ in where pointers come from and where
    /// the shapes go, and in nothing else.
    pub fn frame(&mut self, ui: &mut egui::Ui, input: FrameInput<'_>) {
        if self.theme_dirty {
            super::adapter::apply_style(ui.ctx(), self.theme);
            self.theme_dirty = false;
        }
        // The root Ui from run_ui spans the whole viewport, so its max_rect IS the screen —
        // one source of truth for both hosts, no Option to fall back from.
        let screen = sp_rect(ui.max_rect());
        self.recognizer.set_viewport(screen);

        // 1. gestures: pointers in, intents out (the toolkit never interprets input itself)
        let mut intents = Vec::new();
        for ev in input.pointers {
            intents.extend(self.recognizer.push(*ev));
        }
        intents.extend(self.recognizer.advance(input.now_ms));
        for s in self.recognizer.drain_suppressions() {
            self.push_log(format!(
                "[input] t={}ms pointer {} suppressed: {}",
                s.t_ms, s.pointer, s.reason
            ));
        }

        // 2. layout FIRST, and the breakpoint refusal rule — routing an intent needs the canvas
        //    rect and the effective mode, so the layout is computed before intents are dispatched.
        let mut layout = shell::compute(screen, &self.state);
        self.design_refused = layout.design_refused;
        if layout.design_refused {
            // Below the tablet breakpoint, Design is REFUSED — not squeezed (§14.2). Perform
            // mode reflows and runs. The banner in draw_canvas says so in words, and the log
            // records the transition once, so the refusal is an event and not a mood.
            if self.state.mode == ShellMode::Design {
                self.push_log(
                    "DESIGN MODE REFUSED: viewport below the tablet breakpoint; running Perform"
                        .to_string(),
                );
            }
            self.state.mode = ShellMode::Perform;
            self.state.rail_collapsed = true;
            self.state.inspector_collapsed = true;
            self.state.dock_collapsed = true;
            layout = shell::compute(screen, &self.state);
        }

        // 3. intents → state changes. Each intent goes to the canvas (the focused surface in
        //    Design mode) or to the shell chrome, decided by where it landed / what is in flight.
        //    The canvas reads the PREVIOUS frame's layout — in immediate mode that is what was on
        //    screen when the finger came down, the same convention the shell's registry uses.
        let canvas_rect = layout.canvas;
        let ctx = ConnectContext::no_adapters(Phase::Zero);
        for intent in &intents {
            if self.route_to_canvas(*intent, &layout) {
                for ev in self.canvas.on_intent(
                    &mut self.graph,
                    *intent,
                    &self.canvas_layout,
                    canvas_rect,
                    &ctx,
                ) {
                    if let CanvasEvent::RenderWav = ev {
                        self.render_canvas_to_wav();
                    } else {
                        self.push_log(ev.message());
                    }
                }
            } else {
                self.apply(*intent, input.now_ms);
            }
        }

        // 3b. the browser's provisional text path: while the sheet is modal, typed characters,
        //     Backspace, Enter, Escape and the arrows go to it. This is an INPUT-EVENT feed, not
        //     an egui widget — the wrap-egui rule holds (no widget owns input behind the
        //     recogniser's back); a real text-entry surface (rename, search history) is its own
        //     future increment. Headless drivers call `browser_set_query` directly.
        if self.canvas.browser().is_some() {
            self.feed_browser_keys(ui, canvas_rect);
        }

        // 4. Recompute the shell layout so this frame's toggles are reflected in what we draw and
        //    in `last_layout` (step 2's effective layout predates the intents, and exists only to
        //    give routing a canvas rect and mode). `design_refused` is left as step 2 set it — the
        //    recompute now sees the forced Perform mode and would otherwise clear the banner.
        //    Then recompute the canvas layout from the current graph + camera inside the final
        //    canvas rect: this is what we draw this frame and what next frame's hit-testing reads.
        let layout = shell::compute(screen, &self.state);
        self.canvas_layout =
            sparq_ui::canvas::layout::compute(&self.graph, &self.canvas.camera, layout.canvas);

        // 5. the inspector geometry for the CURRENT selection: exactly one node selected and the
        //    panel open, else None (multi-select inspects nothing in v0 — a param edit needs one
        //    unambiguous target). Computed here so next frame's routing and this frame's paint
        //    read the same rects — what you see is what you touch. The panel rect minus its
        //    header band is the content the rows live in.
        let inspector = if self.state.mode == ShellMode::Design {
            layout.inspector.and_then(|panel| {
                let mut sel = self.canvas.selection.nodes.iter();
                let (Some(&id), None) = (sel.next(), sel.next()) else { return None };
                let node = self.graph.node(id)?;
                let hb = LAYOUT_SHELL_PANEL_HEADER_HEIGHT as f32;
                let content = Rect::new(Vec2::new(panel.min.x, panel.min.y + hb), panel.max);
                Some(sparq_ui::canvas::inspector::compute(node, content))
            })
        } else {
            None
        };
        self.canvas.set_inspector(inspector);

        self.draw(ui, &layout);
        self.last_layout = Some(layout);
    }

    /// The browser's keyboard feed (step 3b of `frame`): characters append to the query,
    /// Backspace deletes, the arrows move the selection, Enter spawns it, Escape closes. Every
    /// action logs through the same event path the gestures use — one voice for the shell.
    fn feed_browser_keys(&mut self, ui: &egui::Ui, canvas_rect: Rect) {
        let mut q = self.canvas.browser().map_or(String::new(), |b| b.query().to_string());
        let mut query_changed = false;
        let mut nav = 0i64;
        let mut confirm = false;
        let mut escape = false;
        ui.input(|i| {
            for ev in &i.raw.events {
                match ev {
                    egui::Event::Text(t) => {
                        // Control characters arrive as Text on some backends; the query is printables only.
                        if t.chars().all(|c| !c.is_control()) {
                            q.push_str(t);
                            query_changed = true;
                        }
                    },
                    egui::Event::Key { key, pressed: true, .. } => match key {
                        egui::Key::Backspace => {
                            q.pop();
                            query_changed = true;
                        },
                        egui::Key::ArrowUp => nav -= 1,
                        egui::Key::ArrowDown => nav += 1,
                        egui::Key::Enter => confirm = true,
                        egui::Key::Escape => escape = true,
                        _ => {},
                    },
                    _ => {},
                }
            }
        });
        let mut ev = Vec::new();
        if escape {
            ev.extend(self.canvas.browser_close());
        } else {
            if query_changed {
                ev.extend(self.canvas.browser_set_query(&q));
            }
            if nav != 0 {
                self.canvas.browser_navigate(nav, canvas_rect);
            }
            if confirm {
                ev.extend(self.canvas.browser_confirm(&mut self.graph));
            }
        }
        for e in ev {
            self.push_log(e.message());
        }
    }

    /// The audit over the elements registered in the last drawn frame.
    #[must_use]
    pub fn audit_report(&self) -> sparq_ui::audit::AuditReport {
        sparq_ui::audit::audit(&self.audit_elements, self.state.mode)
    }

    /// The visible diagnostic log (newest last).
    #[must_use]
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// Where a registered element sat in the last drawn frame. Synthetic-input drivers (the
    /// headless smoke suite) use this to tap buttons without duplicating layout arithmetic —
    /// they aim at exactly the pixels the audit measured.
    #[must_use]
    pub fn rect_of(&self, id: &str) -> Option<Rect> {
        self.registry.iter().find(|r| r.id == id).map(|r| r.rect)
    }

    /// The canvas layout computed in the last drawn frame. Synthetic-input drivers (the headless
    /// smoke suite) read node and port screen positions from it to aim gestures at the exact
    /// pixels the user would touch, without duplicating the layout arithmetic.
    #[must_use]
    pub fn canvas_layout(&self) -> &CanvasLayout {
        &self.canvas_layout
    }

    /// Force a mode (headless drivers and tests use this; the window uses the toggle button).
    pub fn set_mode(&mut self, mode: ShellMode) {
        self.state.mode = mode;
        if mode == ShellMode::Perform {
            self.state.rail_collapsed = true;
            self.state.inspector_collapsed = true;
            self.state.dock_collapsed = true;
        } else {
            self.state.rail_collapsed = false;
            self.state.inspector_collapsed = false;
            self.state.dock_collapsed = false;
        }
    }

    // ------------------------------------------------------------------ intent handling

    /// Whether an intent belongs to the canvas rather than the shell chrome. The canvas graph is
    /// a Design-mode surface: in Perform mode it is inert (the stage pads own the screen), so
    /// everything routes to the shell. Otherwise, positional intents go to the canvas when they
    /// land in the canvas rect OR on an inspector parameter row (the inspector's slider drags are
    /// canvas operations — the panel is the canvas's parameter surface, increment 3); a drag in
    /// flight stays with whoever started it; and pan / zoom / undo are the canvas's (the panels
    /// do not pan or zoom in increment 1).
    fn route_to_canvas(&self, intent: GestureIntent, layout: &ShellLayout) -> bool {
        if self.state.mode != ShellMode::Design {
            return false;
        }
        let canvas_rect = layout.canvas;
        // An inspector-row hit: inside the panel rect AND on a row the previous frame computed
        // (the same stale-by-one-frame convention the canvas hit-testing uses).
        let inspector_hit = |pos: Vec2| -> bool {
            layout.inspector.is_some_and(|r| r.contains(pos))
                && self.canvas.inspector().is_some_and(|il| il.row_at(pos).is_some())
        };
        match intent {
            GestureIntent::Activate { pos } | GestureIntent::DragStart { pos } => {
                canvas_rect.contains(pos) || inspector_hit(pos)
            },
            GestureIntent::Context { pos } | GestureIntent::DoubleTap { pos } => {
                canvas_rect.contains(pos)
            },
            GestureIntent::DragUpdate { .. } | GestureIntent::DragEnd { .. } => {
                !matches!(self.canvas.interaction, Interaction::Idle)
            },
            GestureIntent::Pan { .. } | GestureIntent::Zoom { .. } | GestureIntent::Undo => true,
            _ => false,
        }
    }

    fn apply(&mut self, intent: GestureIntent, now_ms: u64) {
        match intent {
            GestureIntent::Activate { pos } => {
                // Hit-test against the registry the user was looking at (previous frame — in
                // immediate mode that IS what was on screen when the finger came down).
                match self.registry.iter().rev().find(|r| r.rect.contains(pos)) {
                    Some(hit) => {
                        let (action, id) = (hit.action, hit.id);
                        self.push_log(format!("t={now_ms}ms activate: {id}"));
                        self.run(action);
                    },
                    None => self.push_log(format!("t={now_ms}ms activate: (empty space)")),
                }
            },
            GestureIntent::DoubleTap { .. } => {
                self.push_log(format!("t={now_ms}ms double-tap (zoom-to-fit binds in WO-013)"));
            },
            GestureIntent::Context { pos } => {
                let id = self
                    .registry
                    .iter()
                    .rev()
                    .find(|r| r.rect.contains(pos))
                    .map_or("empty space", |r| r.id);
                self.push_log(format!(
                    "t={now_ms}ms long-press context: {id} (menu binds in WO-013)"
                ));
            },
            GestureIntent::DragStart { .. } => self.push_log(format!("t={now_ms}ms drag start")),
            GestureIntent::DragUpdate { scale, .. } => {
                if (scale - 1.0).abs() > f32::EPSILON {
                    self.push_log(format!("t={now_ms}ms drag update (FINE ×{:.0})", 1.0 / scale));
                }
            },
            GestureIntent::DragFineChanged { active } => {
                self.push_log(format!(
                    "t={now_ms}ms fine resolution {}",
                    if active { "ENGAGED (×10)" } else { "released" }
                ));
            },
            GestureIntent::DragEnd { cancelled, .. } => {
                self.push_log(format!(
                    "t={now_ms}ms drag end{}",
                    if cancelled { " (CANCELLED)" } else { "" }
                ));
            },
            GestureIntent::Pan { delta } => {
                self.push_log(format!("t={now_ms}ms pan ({:.0}, {:.0})", delta.x, delta.y));
            },
            GestureIntent::Zoom { factor, .. } => {
                self.push_log(format!("t={now_ms}ms zoom ×{factor:.3}"));
            },
            GestureIntent::Rotate { delta_rad, .. } => {
                self.push_log(format!("t={now_ms}ms rotate {delta_rad:.3} rad"));
            },
            GestureIntent::Undo => self.push_log(format!("t={now_ms}ms UNDO (3-finger tap)")),
            GestureIntent::AllSoundOff => {
                self.push_log(format!("t={now_ms}ms ALL SOUND OFF (3-finger swipe down)"));
                self.run(Action::Panic);
            },
            GestureIntent::RecoveryMenu => {
                self.push_log(format!("t={now_ms}ms recovery menu (5-finger hold)"));
            },
            GestureIntent::Flick { velocity_px_per_ms, .. } => {
                self.push_log(format!("t={now_ms}ms flick {velocity_px_per_ms:.2} px/ms"));
            },
            GestureIntent::TogglePanel { edge } => match edge {
                Edge::Left => {
                    self.state.rail_collapsed = !self.state.rail_collapsed;
                    self.push_log(format!("t={now_ms}ms edge swipe: rail toggled"));
                },
                Edge::Right => {
                    self.state.inspector_collapsed = !self.state.inspector_collapsed;
                    self.push_log(format!("t={now_ms}ms edge swipe: inspector toggled"));
                },
                Edge::Bottom => {
                    self.state.dock_collapsed = !self.state.dock_collapsed;
                    self.push_log(format!("t={now_ms}ms edge swipe: dock toggled"));
                },
                Edge::Top => self.push_log(format!("t={now_ms}ms edge swipe: top (unbound)")),
            },
        }
    }

    fn run(&mut self, action: Action) {
        match action {
            Action::ToggleMode => {
                let next = match self.state.mode {
                    ShellMode::Design => ShellMode::Perform,
                    ShellMode::Perform => ShellMode::Design,
                };
                self.set_mode(next);
                self.push_log(format!("mode: {next:?}"));
            },
            Action::ToggleTheme => {
                self.theme = self.theme.toggled();
                self.theme_dirty = true;
                self.push_log(format!("theme: {:?}", self.theme));
            },
            Action::ToggleRail => {
                self.state.rail_collapsed = !self.state.rail_collapsed;
                self.push_log(format!(
                    "rail: {}",
                    if self.state.rail_collapsed { "collapsed" } else { "expanded" }
                ));
            },
            Action::ToggleInspector => {
                self.state.inspector_collapsed = !self.state.inspector_collapsed;
                self.push_log(format!(
                    "inspector: {}",
                    if self.state.inspector_collapsed { "collapsed" } else { "expanded" }
                ));
            },
            Action::ToggleDock => {
                self.state.dock_collapsed = !self.state.dock_collapsed;
                self.push_log(format!(
                    "dock: {}",
                    if self.state.dock_collapsed { "collapsed" } else { "expanded" }
                ));
            },
            Action::Play => {
                self.push_log("transport: PLAY (engine binding: WO-007/008)".to_string())
            },
            Action::Stop => self.push_log("transport: STOP".to_string()),
            Action::Panic => {
                self.push_log("PANIC: all sound off (engine binding: WO-007/008)".to_string());
            },
        }
    }

    fn push_log(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > LOG_LINES {
            let excess = self.log.len() - LOG_LINES;
            self.log.drain(0..excess);
        }
    }

    /// The RENDER WAV menu action: resolve the master (explicit or the documented default rule),
    /// render the drawn patch through the registry + executor, write `canvas-render.wav` in the
    /// working directory, and log the evidence. Every failure path logs a sentence with the
    /// remedy — the shell explains itself, in words.
    fn render_canvas_to_wav(&mut self) {
        const OUT: &str = "canvas-render.wav";
        let Some(master) = self.canvas.resolve_master(&self.graph) else {
            self.push_log(
                "render REFUSED: the patch has no module with an audio output — add one (e.g. \
                 util/gain) or long-press a node and SET MASTER"
                    .to_string(),
            );
            return;
        };
        let result = crate::bridge::render_wav(
            &self.graph,
            master,
            &self.modules,
            std::path::Path::new(OUT),
        );
        match result {
            Ok(ev) => self
                .push_log(format!("rendered {} s to `{OUT}`: {ev}", crate::bridge::RENDER_SECONDS)),
            Err(e) => self.push_log(format!("render REFUSED: {e}")),
        }
    }

    // ------------------------------------------------------------------ drawing

    fn draw(&mut self, ui: &mut egui::Ui, layout: &ShellLayout) {
        self.registry.clear();
        self.audit_elements.clear();
        let pal = Palette::for_theme(self.theme);
        let p = ui.painter();

        // The canvas is the ground; panels sit on it.
        self.draw_canvas(p, layout, &pal);
        self.draw_top_bar(p, layout, &pal);
        if let Some(rail) = layout.rail {
            self.draw_rail(p, rail, &pal);
        }
        if let Some(insp) = layout.inspector {
            self.draw_inspector(p, insp, &pal);
        }
        if let Some(dock) = layout.dock {
            self.draw_dock(p, dock, &pal);
        }
    }

    /// Draw one interactive control and register it for hit-testing AND the audit. Every
    /// interactive pixel in the shell goes through this function — that is what makes the audit
    /// complete by construction rather than by discipline.
    fn button(&mut self, p: &Painter, pal: &Palette, b: &Button) {
        let eg = egui_rect(b.rect);
        p.rect_filled(eg, LAYOUT_CORNER_MICRO as u8, pal.ground_panel_alt);
        p.rect_stroke(
            eg,
            LAYOUT_CORNER_MICRO as u8,
            pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32),
            egui::StrokeKind::Middle,
        );
        let color = b.accent.unwrap_or(pal.text_secondary);
        p.text(eg.center(), Align2::CENTER_CENTER, b.label, font_s(), color);
        self.registry.push(Registered { id: b.id, rect: b.rect, class: b.class, action: b.action });
        self.audit_elements.push(InteractiveElement {
            id: b.id.to_string(),
            class: b.class,
            rect: b.rect,
            dense_allowed: b.dense_allowed,
        });
    }

    fn panel_frame(&self, p: &Painter, pal: &Palette, rect: Rect, title: &str) {
        let eg = egui_rect(rect);
        p.rect_filled(eg, LAYOUT_CORNER_PANEL as u8, pal.ground_panel);
        p.rect_stroke(
            eg,
            LAYOUT_CORNER_PANEL as u8,
            pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32),
            egui::StrokeKind::Middle,
        );
        // The title sits in the panel header band (token height), left-padded by the token panel
        // padding. Align2 has no vector arithmetic; the offset belongs on the anchor point.
        let header_mid = LAYOUT_SHELL_PANEL_HEADER_HEIGHT as f32 / 2.0;
        p.text(
            eg.left_top() + egui::vec2(LAYOUT_SPACE_PADDING_PANEL as f32, header_mid),
            Align2::LEFT_CENTER,
            title,
            font_s(),
            pal.text_tertiary,
        );
    }

    fn draw_top_bar(&mut self, p: &Painter, layout: &ShellLayout, pal: &Palette) {
        let eg = egui_rect(layout.top_bar);
        p.rect_filled(eg, LAYOUT_CORNER_NONE as u8, pal.ground_base);
        p.line_segment(
            [eg.left_bottom(), eg.right_bottom()],
            pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32),
        );
        // The wordmark wears the audio accent: in this interface, the accent IS the signal.
        p.text(
            eg.left_center() + egui::vec2(LAYOUT_SPACE_PADDING_PANEL as f32, 0.0),
            Align2::LEFT_CENTER,
            "sparq",
            font_l(),
            pal.audio,
        );
        if self.state.mode == ShellMode::Perform {
            // Perform mode: the top bar is a LABEL, not a control surface. Design controls here
            // would be class-S elements in a mode where nothing below L is interactive — and
            // "Design chrome is not merely hidden, it is not hit-testable" (input-model §4)
            // means: not drawn, not registered. The way back to Design is the XL pad on the
            // canvas, sized for a hand, not a cursor.
            p.text(
                eg.left_center() + egui::vec2(LAYOUT_SPACE_9 as f32 + LAYOUT_SPACE_2 as f32, 0.0),
                Align2::LEFT_CENTER,
                "PERFORM",
                font_m(),
                pal.audio,
            );
            return;
        }

        p.text(
            eg.left_center() + egui::vec2(LAYOUT_SPACE_9 as f32 + LAYOUT_SPACE_2 as f32, 0.0),
            Align2::LEFT_CENTER,
            "phase 0 shell prototype - WO-012",
            font_xs(),
            pal.text_disabled,
        );

        // Right-aligned control cluster. Height is the token rail_button (44 px) inside the
        // 48 px bar; width is label advance + section padding. The advance budget comes off the
        // 8 px space scale (monospace at scale S never exceeds it per char), so no font metrics
        // are hard-coded either.
        let h = LAYOUT_SHELL_RAIL_BUTTON as f32;
        let gap = LAYOUT_SPACE_2 as f32;
        let pad = LAYOUT_SPACE_3 as f32;
        let char_w = LAYOUT_SPACE_2 as f32;
        let labels: [(&str, &str, Action); 5] = [
            (
                "topbar/mode",
                if self.state.mode == ShellMode::Design { "DESIGN" } else { "PERFORM" },
                Action::ToggleMode,
            ),
            (
                "topbar/theme",
                if self.theme == ThemeChoice::PhosphorDark { "HC" } else { "STD" },
                Action::ToggleTheme,
            ),
            ("topbar/rail", "RAIL", Action::ToggleRail),
            ("topbar/inspector", "INSP", Action::ToggleInspector),
            ("topbar/dock", "DOCK", Action::ToggleDock),
        ];
        let y0 = layout.top_bar.min.y + (LAYOUT_SHELL_TOP_BAR_HEIGHT as f32 - h) / 2.0;
        let mut x = layout.top_bar.max.x - pad;
        for (id, label, action) in labels.iter().rev() {
            let w = pad * 2.0 + label.len() as f32 * char_w;
            x -= w;
            let rect = Rect::new(Vec2::new(x, y0), Vec2::new(x + w, y0 + h));
            let mut b = Button::new(id, rect, label, TouchClass::S, *action).dense();
            match action {
                Action::ToggleMode if self.state.mode == ShellMode::Perform => {
                    b = b.accent(pal.audio)
                },
                Action::ToggleTheme if self.theme == ThemeChoice::ContrastHigh => {
                    b = b.accent(pal.selected)
                },
                _ => {},
            }
            self.button(p, pal, &b);
            x -= gap;
        }
    }

    fn draw_rail(&mut self, p: &Painter, rail: Rect, pal: &Palette) {
        let eg = egui_rect(rail);
        p.rect_filled(eg, LAYOUT_CORNER_NONE as u8, pal.ground_panel);
        p.line_segment(
            [eg.right_top(), eg.right_bottom()],
            pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32),
        );
        // 44 px buttons in a 56 px rail: the token pair (rail_width, rail_button) exists exactly
        // for this. Sections top-down: transport, mode, browsers, diagnostics (plan §14.2).
        let b = LAYOUT_SHELL_RAIL_BUTTON as f32;
        let gap = LAYOUT_SPACE_2 as f32;
        let x0 = rail.min.x + (rail.width() - b) / 2.0;
        let mut y = rail.min.y + gap;
        let entries: [(&str, &str, Action); 6] = [
            ("rail/transport/play", "PLAY", Action::Play),
            ("rail/transport/stop", "STOP", Action::Stop),
            ("rail/transport/panic", "PANIC", Action::Panic),
            ("rail/mode/toggle", "MODE", Action::ToggleMode),
            ("rail/browser", "MODS", Action::ToggleDock),
            ("rail/diagnostics", "DIAG", Action::ToggleDock),
        ];
        for (id, label, action) in entries {
            let rect = Rect::new(Vec2::new(x0, y), Vec2::new(x0 + b, y + b));
            // Panic is red before you press it: the one control whose colour carries urgency
            // rather than signal class — and it stays redundant with the word PANIC, because
            // nothing may be encoded by colour alone (look-board rule).
            let mut btn = Button::new(id, rect, label, TouchClass::S, action);
            match action {
                Action::Panic => btn = btn.accent(pal.error),
                Action::Play => btn = btn.accent(pal.playing),
                _ => {},
            }
            self.button(p, pal, &btn);
            y += b + gap;
        }
    }

    fn draw_canvas(&mut self, p: &Painter, layout: &ShellLayout, pal: &Palette) {
        let eg = egui_rect(layout.canvas);
        p.rect_filled(eg, LAYOUT_CORNER_NONE as u8, pal.ground_canvas);

        if self.state.mode == ShellMode::Design {
            // The graph, drawn by the canvas painter from the computed layout (WO-013). The
            // painter registers node/port/menu touch targets into the same audit list the chrome
            // uses, so the canvas is measured by the same gate as everything else.
            let ctx = ConnectContext::no_adapters(Phase::Zero);
            canvas_ui::draw(
                p,
                pal,
                &self.graph,
                &self.canvas,
                &self.canvas_layout,
                layout.canvas,
                &ctx,
                &mut self.audit_elements,
            );
            // A one-line affordance hint above the intent log band (the two never collide).
            p.text(
                eg.left_bottom()
                    + egui::vec2(
                        LAYOUT_SPACE_PADDING_PANEL as f32,
                        -(LAYOUT_SPACE_9 as f32 + LAYOUT_SPACE_6 as f32),
                    ),
                Align2::LEFT_BOTTOM,
                "drag port to patch - long-press empty canvas: ADD MODULE - long-press a node: menu - drag a wire end: re-patch - double-tap fits",
                font_xs(),
                pal.text_disabled,
            );
        } else {
            // Perform: XL macro pads are the ONLY interactive things on screen (input-model §4:
            // nothing below class L is interactive in Perform mode). Pad size: the token minimum
            // (96) plus a spacing step, inside the documented typical band (120-160).
            let pad = LAYOUT_TOUCH_STAGE_MIN_MACRO_PAD as f32 + LAYOUT_SPACE_5 as f32;
            let gap = LAYOUT_SPACE_5 as f32;
            let total = pad * 3.0 + gap * 2.0;
            let mut x = layout.canvas.center().x - total / 2.0;
            let y = layout.canvas.center().y - pad / 2.0 - gap;
            let pads: [(&str, &str, Action, Color32); 3] = [
                ("perform/play", "PLAY", Action::Play, pal.playing),
                ("perform/stop", "STOP", Action::Stop, pal.text_primary),
                ("perform/panic", "PANIC", Action::Panic, pal.error),
            ];
            for (id, label, action, col) in pads {
                let rect = Rect::new(Vec2::new(x, y), Vec2::new(x + pad, y + pad));
                self.button(
                    p,
                    pal,
                    &Button::new(id, rect, label, TouchClass::XL, action).accent(col),
                );
                x += pad + gap;
            }
            // ...and one way back to Design — also XL, because it is interactive.
            let back = Rect::new(
                Vec2::new(layout.canvas.center().x - pad / 2.0, y + pad + gap),
                Vec2::new(layout.canvas.center().x + pad / 2.0, y + pad * 2.0 + gap),
            );
            self.button(
                p,
                pal,
                &Button::new("perform/mode", back, "DESIGN", TouchClass::XL, Action::ToggleMode)
                    .accent(pal.audio),
            );
        }

        // Fine-resolution indicator while engaged (§14.3 rule 5: the assist must be visible).
        if self.recognizer.fine_mode() {
            p.text(
                eg.right_top()
                    + egui::vec2(
                        -(LAYOUT_SPACE_PADDING_PANEL as f32),
                        LAYOUT_SPACE_PADDING_PANEL as f32,
                    ),
                Align2::RIGHT_TOP,
                format!("FINE x{}", LAYOUT_TOUCH_GESTURE_FINE_RESOLUTION_FACTOR),
                font_m(),
                pal.audio,
            );
        }

        // The refusal banner: stated in words, with the remedy (§14.2 rule 1, accessibility §7.8).
        if self.design_refused {
            p.text(
                eg.center_top() + egui::vec2(0.0, LAYOUT_SPACE_PADDING_SECTION as f32),
                Align2::CENTER_TOP,
                "DESIGN MODE REFUSED: viewport below the 1280x800 tablet breakpoint - running Perform",
                font_s(),
                pal.warning,
            );
        }

        // The intent/diagnostic log: the shell shows what it understood. "An input that did
        // nothing must always be explainable" (input-model §3) — here is where it is explained.
        let line_h = LAYOUT_SPACE_4 as f32;
        let mut ly = layout.canvas.max.y - LAYOUT_SPACE_PADDING_PANEL as f32;
        for line in self.log.iter().rev().take(LOG_LINES) {
            p.text(
                egui::pos2(layout.canvas.min.x + LAYOUT_SPACE_PADDING_PANEL as f32, ly),
                Align2::LEFT_BOTTOM,
                line,
                font_xs(),
                pal.text_tertiary,
            );
            ly -= line_h;
        }
    }

    fn draw_inspector(&mut self, p: &Painter, insp: Rect, pal: &Palette) {
        self.panel_frame(p, pal, insp, "INSPECTOR");
        // The collapse control lives in the 40 px panel header — below the 44 px floor, so it
        // rides the Design-mode dense exception (non-destructive, badged in the audit report).
        // In Perform mode the whole panel is gone, not merely hidden.
        let hb = LAYOUT_SHELL_PANEL_HEADER_HEIGHT as f32;
        let collapse = Rect::new(
            Vec2::new(insp.max.x - hb, insp.min.y),
            Vec2::new(insp.max.x, insp.min.y + hb),
        );
        self.button(
            p,
            pal,
            &Button::new(
                "inspector/collapse",
                collapse,
                "[x]",
                TouchClass::S,
                Action::ToggleInspector,
            )
            .dense(),
        );

        // Increment 3: ONE selected node → its parameters, as touch sliders. Geometry is the
        // computed `InspectorLayout` the gesture path hit-tests — the row you see is the row you
        // touch. Anything else (no selection, multi-selection) says so in words.
        let il = self.canvas.inspector().cloned();
        let node = il.as_ref().and_then(|l| self.graph.node(l.node).cloned());
        if let (Some(il), Some(node)) = (il, node) {
            p.text(
                egui::pos2(il.title.min.x, il.title.center().y),
                Align2::LEFT_CENTER,
                node.title(),
                font_s(),
                pal.text_primary,
            );
            p.text(
                egui::pos2(il.title.max.x, il.title.center().y),
                Align2::RIGHT_CENTER,
                node.spec.module_id.as_str(),
                font_xs(),
                pal.text_tertiary,
            );
            p.line_segment(
                [
                    egui::pos2(il.title.min.x, il.title.max.y),
                    egui::pos2(il.title.max.x, il.title.max.y),
                ],
                pal.hairline(pal.hairline_faint, LAYOUT_STROKE_HAIRLINE as f32),
            );
            if il.rows.is_empty() {
                p.text(
                    egui::pos2(il.title.min.x, il.title.max.y + LAYOUT_SPACE_4 as f32),
                    Align2::LEFT_TOP,
                    "this module declares no parameters",
                    font_s(),
                    pal.text_disabled,
                );
            }
            for row in &il.rows {
                let Some(desc) = node.spec.params.get(row.index) else { continue };
                // Rows below the panel bottom are not drawn — and the core refuses to hit-test
                // them, so the clip is honest in both directions.
                if row.track.min.y >= insp.max.y {
                    break;
                }
                let value = node.param_value(row.index).unwrap_or(desc.default as f32);
                p.text(
                    egui::pos2(row.label.min.x, row.label.center().y),
                    Align2::LEFT_CENTER,
                    desc.name.as_str(),
                    font_s(),
                    if row.editable { pal.text_secondary } else { pal.text_disabled },
                );
                if row.editable {
                    let mid_y = row.track.center().y;
                    let (a, b) =
                        (egui::pos2(row.track.min.x, mid_y), egui::pos2(row.track.max.x, mid_y));
                    p.line_segment(
                        [a, b],
                        pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32),
                    );
                    let kx = sparq_ui::canvas::inspector::knob_x(desc, row.track, value);
                    let knob = egui::pos2(kx, mid_y);
                    p.line_segment(
                        [a, knob],
                        Stroke::new(LAYOUT_STROKE_SIGNAL as f32, pal.selected),
                    );
                    p.circle_filled(knob, LAYOUT_TOUCH_PORT_RADIUS as f32, pal.selected);
                    p.circle_stroke(
                        knob,
                        LAYOUT_TOUCH_PORT_RADIUS as f32,
                        pal.hairline(pal.hairline_strong, LAYOUT_STROKE_HAIRLINE as f32),
                    );
                }
                p.text(
                    egui::pos2(row.value.max.x, row.value.center().y),
                    Align2::RIGHT_CENTER,
                    sparq_ui::canvas::inspector::value_text(desc, value),
                    font_s(),
                    if row.editable { pal.text_primary } else { pal.text_disabled },
                );
                // The whole row span is the touch target (the drawn track is thin; the row is
                // 44 px — the port-capture trick applied to sliders).
                self.audit_elements.push(InteractiveElement {
                    id: format!("inspector/param/{}/{}", il.node, row.index),
                    class: TouchClass::S,
                    rect: Rect::new(row.label.min, row.value.max),
                    dense_allowed: false,
                });
            }
            return;
        }

        let mut y = insp.min.y + hb + LAYOUT_SPACE_PADDING_SECTION as f32;
        for line in [
            "select ONE node - its params appear here",
            "ports    - WO-008 graph edges",
            "mapping  - WO-009 controller map",
            "analysis - WO-014",
        ] {
            p.text(
                egui::pos2(insp.min.x + LAYOUT_SPACE_PADDING_PANEL as f32, y),
                Align2::LEFT_TOP,
                line,
                font_s(),
                pal.text_disabled,
            );
            y += LAYOUT_SPACE_5 as f32;
        }
    }

    fn draw_dock(&mut self, p: &Painter, dock: Rect, pal: &Palette) {
        self.panel_frame(p, pal, dock, "DOCK");
        let hb = LAYOUT_SHELL_PANEL_HEADER_HEIGHT as f32;
        let collapse = Rect::new(
            Vec2::new(dock.max.x - hb, dock.min.y),
            Vec2::new(dock.max.x, dock.min.y + hb),
        );
        self.button(
            p,
            pal,
            &Button::new("dock/collapse", collapse, "[x]", TouchClass::S, Action::ToggleDock)
                .dense(),
        );

        // Tab labels drawn DISABLED until WO-013 gives them content. Drawing them interactive
        // and doing nothing would be a lie; disabled says "coming" honestly (and keeps them out
        // of the audit, which only measures things you can actually touch).
        let mut x = dock.min.x + LAYOUT_SPACE_PADDING_PANEL as f32 + LAYOUT_SPACE_9 as f32;
        for tab in ["MODULES", "LIBRARY", "STREAMS", "SCENES", "JOURNAL"] {
            p.text(
                egui::pos2(x, dock.min.y + hb + LAYOUT_SPACE_2 as f32),
                Align2::LEFT_CENTER,
                tab,
                font_xs(),
                pal.text_disabled,
            );
            x += LAYOUT_SPACE_9 as f32;
        }
        let body_y = dock.min.y + hb + LAYOUT_SPACE_4 as f32 + LAYOUT_SPACE_4 as f32;
        p.text(
            egui::pos2(dock.min.x + LAYOUT_SPACE_PADDING_PANEL as f32, body_y),
            Align2::LEFT_TOP,
            "module browser lands with WO-013; until then this panel proves the shell layout,",
            font_s(),
            pal.text_tertiary,
        );
        p.text(
            egui::pos2(
                dock.min.x + LAYOUT_SPACE_PADDING_PANEL as f32,
                body_y + LAYOUT_SPACE_4 as f32,
            ),
            Align2::LEFT_TOP,
            "the dock collapse/expand gesture path, and the audit's dense-badge exception.",
            font_s(),
            pal.text_tertiary,
        );
    }
}
