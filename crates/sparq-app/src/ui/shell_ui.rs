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
use sparq_ui::canvas::camera::Lod;
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
    /// Transport: start the live audio session (WO-012 increment 2 — the engine binding the
    /// stub was named for). Refusals are sentences with remedies, like every other voice.
    Play,
    /// Transport: stop the live session; the evidence line carries the measured numbers.
    Stop,
    /// All sound off — stops the live session immediately. The only button that is red before
    /// you press it.
    Panic,
    /// Open the module browser at the canvas centre — the rail's ADD cross (WO-012 inc 3).
    AddModule,
    /// Spawn the dock's catalogue card at the canvas centre (WO-012 inc 3): the dock card grid's
    /// tap, riding the same op path as the browser's row tap.
    SpawnFromDock(usize),
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
    id: String,
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
    /// The live audio session (WO-012 increment 2): `Some` while PLAY is running. The PLAY /
    /// STOP / PANIC actions bind to it; `frame` gives it the op→sync door and the per-frame
    /// meter drain; the audit drives it on the manual null backend. `None` = nothing playing.
    pub live: Option<crate::ui::live::LiveSession>,
    /// The "no master while live" refusal was already said this episode — every frame saying
    /// it would flood the log; the transition is the event (the shell's one-voice rule).
    live_master_lost: bool,
    /// Last frame's computed canvas layout — hit-testing an activation uses the frame the user
    /// actually saw, exactly like the shell's `registry`.
    canvas_layout: CanvasLayout,
}

/// One frame's keyboard input, normalised: the five facts the modal sheets consume. A value,
/// parsed once per frame from the toolkit's raw events (`ShellUi::read_keys`) — the rename entry
/// and the browser query ride the same batch, which is what makes the text-entry path shared
/// rather than two provisional feeds.
#[derive(Clone, Debug, Default)]
struct KeyBatch {
    /// Printable characters typed this frame.
    text: String,
    /// Backspace presses.
    backs: usize,
    /// Arrow navigation, signed (up = -1, down = +1 per press).
    nav: i64,
    /// Enter was pressed.
    enter: bool,
    /// Escape was pressed.
    escape: bool,
}

/// Per-frame inputs the host (window or headless driver) supplies.
pub struct FrameInput<'a> {
    /// Normalised pointer events since the last frame (winit adapter or synthetic).
    pub pointers: &'a [PointerEvent],
    /// Monotonic milliseconds — the recogniser never reads a clock itself.
    pub now_ms: u64,
    /// Intents the host adapter synthesises OUTSIDE the recogniser (WO-012 increment 4): a
    /// mouse right-click is a `Context` without the 350 ms hold, a wheel is a `Pan` without a
    /// second finger. The vocabulary is the recogniser's own — the adapter only knows which
    /// hardware meant it; touch and mouse land in the same intent table.
    pub extras: &'a [GestureIntent],
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
            live: None,
            live_master_lost: false,
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
        // The host's synthesised intents (right-click, wheel) join the recognised ones: one
        // table, one routing, no parallel semantics.
        intents.extend(input.extras.iter().copied());
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

        // 3b. the modal sheets' keyboard path (increment 5 unified it): while a sheet is modal,
        //     typed characters, Backspace, Enter, Escape and the arrows go to it — the rename
        //     entry first (the deepest modal), then the browser. This is an INPUT-EVENT feed,
        //     not an egui widget — the wrap-egui rule holds (no widget owns input behind the
        //     recogniser's back). The parse happens ONCE (`read_keys`); both sheets consume the
        //     same normalised batch, which is the shared text-entry surface the browser's
        //     provisional feed was declared to be waiting for. Headless drivers call
        //     `browser_set_query` / `rename_set_text` directly.
        self.feed_modal_keys(ui, canvas_rect);

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
                // The port-summary strip (increment 3, the mockup's dot column laid horizontal)
                // sits between the fixed header and the first param row: the rows' geometry
                // includes it, so what you see is what `row_at` lets you touch.
                let content = Rect::new(
                    Vec2::new(panel.min.x, panel.min.y + hb + LAYOUT_SPACE_4 as f32),
                    panel.max,
                );
                // The stored scroll rides along — and `compute_at` clamps it, so the layout that
                // comes back is the truth `set_inspector` stores (one copy of the clamp).
                let scroll = self.canvas.inspector_scroll(id);
                Some(sparq_ui::canvas::inspector::compute_at(node, content, scroll))
            })
        } else {
            None
        };
        self.canvas.set_inspector(inspector);

        // 5b. the LIVE session (WO-012 increment 2), after this frame's edits and before the
        //     draw: health first (a dead stream ends the session in words and the canvas is
        //     untouched), then the one op→sync door, then the bounded drain that fills the wire
        //     levels from the engine's OWN rings — the continuous half of live wire levels.
        self.sync_live();

        self.draw(ui, &layout);
        self.last_layout = Some(layout);
    }

    /// The per-frame live block: health → sync → drain. Field-split borrows (live, graph,
    /// modules, canvas, log) keep it one function; every line the operator sees is pushed
    /// through the session's own honesty discipline.
    fn sync_live(&mut self) {
        // Health: `Removed` / `Failed` end the session with ONE honest line (D3) — the stream
        // object stays valid, so the evidence line still reads its measured counters.
        let failure = self.live.as_ref().and_then(|s| {
            let st = s.health();
            match st {
                sparq_kernel::hal::StreamState::Removed
                | sparq_kernel::hal::StreamState::Failed => {
                    Some((st, s.last_error().unwrap_or_else(|| "no driver message".to_string())))
                },
                _ => None,
            }
        });
        if let Some((st, err)) = failure {
            if let Some(dead) = self.live.take() {
                self.push_log(format!(
                    "live: the stream is {st:?} — {err}; the session stopped and the canvas is \
                     untouched"
                ));
                dead.stop("STOP", &mut self.log);
            }
        }
        let Some(s) = self.live.as_mut() else { return };
        // The op→sync door reads the CURRENT master: a handover is structural even with an
        // empty ledger (D7). No master while live (the operator deleted the output node): the
        // ledger is NOT taken — it waits for a patch the engine can hear again — and the
        // refusal is said ONCE per episode, because every frame saying it would flood the log.
        let Some(master) = self.canvas.resolve_master(&self.graph) else {
            if !self.live_master_lost {
                self.live_master_lost = true;
                self.push_log(
                    "live: the patch has no module with an audio output — the session keeps \
                     playing the last good patch; the edits wait on the canvas"
                        .to_string(),
                );
            }
            return;
        };
        self.live_master_lost = false;
        let changes = self.canvas.take_patch_changes();
        s.sync(&self.graph, &self.modules, master, &changes, &mut self.log);
        // The drain: last-wins per (node, port), bounded per frame (D11). The painter path is
        // the increment-4/5 one — this swaps the SOURCE, not the drawing.
        let levels = s.drain().clone();
        self.canvas.levels = levels;
    }

    /// Start the live session (PLAY, and the audit's hermetic path with explicit options).
    /// Every refusal is a sentence with the remedy — the shell explains itself.
    pub fn start_live_with(&mut self, opts: crate::ui::live::LiveOptions) {
        if self.live.is_some() {
            self.push_log(
                "PLAY: already running — STOP first (one stream at a time; a second would be a \
                 second claim on the device)"
                    .to_string(),
            );
            return;
        }
        let Some(master) = self.canvas.resolve_master(&self.graph) else {
            self.push_log(
                "PLAY REFUSED: the patch has no module with an audio output — add one (e.g. \
                 util/gain) or long-press a node and SET MASTER"
                    .to_string(),
            );
            return;
        };
        // The session is built from the CURRENT graph — ledger entries from before PLAY are
        // history the executor already contains; taking them keeps the first sync honest.
        self.canvas.take_patch_changes();
        match crate::ui::live::LiveSession::start(
            &self.graph,
            master,
            &self.modules,
            opts,
            &mut self.log,
        ) {
            Ok(session) => self.live = Some(session),
            Err(e) => self.push_log(format!("PLAY REFUSED: {e}")),
        }
        self.live_master_lost = false;
    }

    /// Stop the live session (STOP and PANIC — a stopped stream IS all sound off; PANIC's word
    /// keeps its promise without inventing a second mechanism). The evidence line carries the
    /// measured numbers, read not invented.
    fn stop_live(&mut self, verb: &str) {
        self.live_master_lost = false;
        match self.live.take() {
            Some(session) => session.stop(verb, &mut self.log),
            None => self.push_log(format!("{verb}: nothing is playing")),
        }
    }

    /// Feed the open modal sheet from this frame's keyboard events (step 3b of `frame`). The
    /// rename entry is the deepest modal and takes the whole batch; the browser takes it when no
    /// rename is open; with neither open the keys belong to the window, not to the shell. Every
    /// action logs through the same event path the gestures use — one voice for the shell.
    fn feed_modal_keys(&mut self, ui: &egui::Ui, canvas_rect: Rect) {
        if self.canvas.rename().is_none() && self.canvas.browser().is_none() {
            return;
        }
        let keys = Self::read_keys(ui);
        let mut ev = Vec::new();
        if self.canvas.rename().is_some() {
            // The rename sheet: characters and Backspace edit the buffer (silently — the sheet
            // SHOWS the buffer with its caret), Enter commits, Escape cancels. The arrows mean
            // nothing to a single-line buffer and are declined without a log line.
            if !keys.text.is_empty() {
                self.canvas.rename_insert(&keys.text);
            }
            for _ in 0..keys.backs {
                self.canvas.rename_backspace();
            }
            if keys.escape {
                ev.extend(self.canvas.rename_cancel());
            } else if keys.enter {
                ev.extend(self.canvas.rename_commit(&mut self.graph));
            }
        } else if let Some(b) = self.canvas.browser() {
            let mut q = b.query().to_string();
            let mut query_changed = false;
            if !keys.text.is_empty() {
                q.push_str(&keys.text);
                query_changed = true;
            }
            if keys.backs > 0 {
                for _ in 0..keys.backs {
                    q.pop();
                }
                query_changed = true;
            }
            if keys.escape {
                ev.extend(self.canvas.browser_close());
            } else {
                if query_changed {
                    ev.extend(self.canvas.browser_set_query(&q));
                }
                if keys.nav != 0 {
                    self.canvas.browser_navigate(keys.nav, canvas_rect);
                }
                if keys.enter {
                    ev.extend(self.canvas.browser_confirm(&mut self.graph));
                }
            }
        }
        for e in ev {
            self.push_log(e.message());
        }
    }

    /// The toolkit half of the shared text entry: egui raw events → the five facts both modal
    /// sheets consume (printable text, backspaces, arrow navigation, Enter, Escape). Parsing
    /// lives here, in the wrap-egui layer; the entry MODEL (`sparq_ui::canvas::entry`) and the
    /// sheets never see a toolkit type.
    fn read_keys(ui: &egui::Ui) -> KeyBatch {
        let mut keys = KeyBatch::default();
        ui.input(|i| {
            for ev in &i.raw.events {
                match ev {
                    egui::Event::Text(t) => {
                        // Control characters arrive as Text on some backends; printables only.
                        let clean: String = t.chars().filter(|c| !c.is_control()).collect();
                        keys.text.push_str(&clean);
                    },
                    egui::Event::Key { key, pressed: true, .. } => match key {
                        egui::Key::Backspace => keys.backs += 1,
                        egui::Key::ArrowUp => keys.nav -= 1,
                        egui::Key::ArrowDown => keys.nav += 1,
                        egui::Key::Enter => keys.enter = true,
                        egui::Key::Escape => keys.escape = true,
                        _ => {},
                    },
                    _ => {},
                }
            }
        });
        keys
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
                        let (action, id) = (hit.action, hit.id.clone());
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
                    .map_or("empty space", |r| r.id.as_str());
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
            GestureIntent::Pan { delta, .. } => {
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
            Action::Play => self.start_live_with(crate::ui::live::LiveOptions::default()),
            Action::Stop => self.stop_live("STOP"),
            Action::Panic => {
                // PANIC's promise is "all sound off" — a stopped stream IS all sound off, and
                // the evidence line that follows carries the measured numbers.
                self.push_log("PANIC: all sound off".to_string());
                self.stop_live("PANIC");
            },
            Action::AddModule => {
                if let Some(rect) = self.last_layout.as_ref().map(|l| l.canvas) {
                    let center = rect.center();
                    for ev in self.canvas.browser_open_at(center, rect) {
                        self.push_log(ev.message());
                    }
                }
            },
            Action::SpawnFromDock(i) => {
                if let Some(rect) = self.last_layout.as_ref().map(|l| l.canvas) {
                    let Some(id) = self.canvas.catalog().get(i).map(|it| it.spec.module_id.clone())
                    else {
                        self.push_log(format!(
                            "dock card {i} is not in the catalogue — the dock cannot offer what                              is not installed (defect #58)"
                        ));
                        return;
                    };
                    let world = self.canvas.camera.to_world(rect.center(), rect);
                    for ev in self.canvas.spawn_module(&mut self.graph, &id, world) {
                        self.push_log(ev.message());
                    }
                }
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
            Ok(ev) => {
                // Live wire levels (WO-013 increment 4): the patch has now really rendered, so
                // refresh the canvas's per-node levels from the executor's meters. The wires light
                // with the actual signal — never synthesised from canvas data. A continuous
                // play-time refresh rides the same field from `SharedEngine::read_meters` when the
                // live HAL stream routes through it (device track); this is the offline half.
                match crate::bridge::node_levels(&self.graph, master, &self.modules) {
                    Ok(levels) => self.canvas.levels = levels,
                    Err(e) => self.push_log(format!("wire levels unavailable: {e}")),
                }
                self.push_log(format!(
                    "rendered {} s to `{OUT}`: {ev}",
                    crate::bridge::RENDER_SECONDS
                ));
            },
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
        self.registry.push(Registered {
            id: b.id.to_string(),
            rect: b.rect,
            class: b.class,
            action: b.action,
        });
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
        // The mark: a four-ray star in the audio accent (design-mode.svg's logo), then the
        // wordmark — in this interface, the accent IS the signal.
        let logo_c = eg.left_center() + egui::vec2(LAYOUT_SPACE_PADDING_PANEL as f32, 0.0);
        let le = LAYOUT_SPACE_4 as f32;
        let lr = egui::Rect::from_center_size(logo_c, egui::vec2(le, le));
        let lst = Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, pal.audio);
        p.line_segment([egui::pos2(logo_c.x, lr.top()), egui::pos2(logo_c.x, lr.bottom())], lst);
        p.line_segment([egui::pos2(lr.left(), logo_c.y), egui::pos2(lr.right(), logo_c.y)], lst);
        let d = le / 2.0 * 0.7;
        p.line_segment([logo_c - egui::vec2(d, d), logo_c + egui::vec2(d, d)], lst);
        p.line_segment([logo_c - egui::vec2(d, -d), logo_c + egui::vec2(d, -d)], lst);
        p.text(
            eg.left_center() + egui::vec2(LAYOUT_SPACE_PADDING_PANEL as f32 + le, 0.0),
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

        // Section dividers at the mockup's stations (token arithmetic on the 8 px grid), and
        // between them the top-bar DIAGNOSTICS the mockup review names: while a session runs,
        // the negotiated truth with units; at rest, the build stamp. Measured or stamped, never
        // decorative.
        let div1 = eg.min.x + LAYOUT_SPACE_9 as f32 * 2.0 + LAYOUT_SPACE_8 as f32;
        let div2 = eg.min.x + LAYOUT_SPACE_9 as f32 * 8.0;
        for dx in [div1, div2] {
            p.line_segment(
                [
                    egui::pos2(dx, eg.min.y + LAYOUT_SPACE_3 as f32),
                    egui::pos2(dx, eg.max.y - LAYOUT_SPACE_3 as f32),
                ],
                pal.hairline(pal.hairline_faint, LAYOUT_STROKE_HAIRLINE as f32),
            );
        }
        let diag = match self.live.as_ref() {
            Some(session) => session.diag_line(),
            None => crate::stamp::line(),
        };
        p.text(
            egui::pos2(div1 + LAYOUT_SPACE_3 as f32, eg.center().y),
            Align2::LEFT_CENTER,
            diag,
            font_xs(),
            pal.text_tertiary,
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
        // The session status dot rides left of the cluster, paired with its WORD (§4: colour
        // is never the only encoding). At rest: no dot, no word — silence looks silent.
        if self.live.is_some() {
            let word_w = 4.0 * char_w;
            let dot_x = x - gap - word_w - gap - char_w;
            p.text(
                egui::pos2(dot_x + char_w + gap, eg.center().y),
                Align2::LEFT_CENTER,
                "LIVE",
                font_xs(),
                pal.playing,
            );
            p.circle_filled(egui::pos2(dot_x, eg.center().y), char_w / 2.0 + 1.0, pal.playing);
        }
    }

    /// A rail glyph (WO-012 inc 3, the mockup's iconography): geometric, 1 px stroke, drawn in a
    /// 16 px box — the look-board's icon rule. Every glyph rides with its WORD under it (the
    /// operator's decision between the mockup's icon-only rail and the board's label rule).
    fn glyph(p: &Painter, c: egui::Pos2, g: Action, col: Color32) {
        let e = LAYOUT_SPACE_4 as f32;
        let r = egui::Rect::from_center_size(c, egui::vec2(e, e));
        let st = Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, col);
        match g {
            Action::Play => {
                let a = egui::pos2(r.left() + e / 4.0, r.top());
                let b = egui::pos2(r.left() + e / 4.0, r.bottom());
                let t = egui::pos2(r.right() - e / 8.0, c.y);
                p.line(vec![a, b, t, a], st);
            },
            Action::Stop => {
                p.rect_stroke(r.shrink(e / 4.0), 0, st, egui::StrokeKind::Middle);
            },
            Action::Panic => {
                p.circle_stroke(c, e / 2.0 - e / 8.0, st);
            },
            Action::ToggleMode => {
                p.rect_stroke(r.shrink(e / 8.0), 0, st, egui::StrokeKind::Middle);
                p.line_segment(
                    [egui::pos2(r.left() + e / 8.0, c.y), egui::pos2(r.right() - e / 8.0, c.y)],
                    st,
                );
            },
            Action::ToggleDock => {
                p.rect_stroke(
                    egui::Rect::from_min_max(
                        egui::pos2(r.left(), r.top() + e / 4.0),
                        egui::pos2(c.x - e / 8.0, r.bottom()),
                    ),
                    0,
                    st,
                    egui::StrokeKind::Middle,
                );
                p.rect_stroke(
                    egui::Rect::from_min_max(
                        egui::pos2(c.x + e / 8.0, r.top()),
                        egui::pos2(r.right(), r.bottom() - e / 4.0),
                    ),
                    0,
                    st,
                    egui::StrokeKind::Middle,
                );
            },
            Action::ToggleInspector => {
                for i in 0..3 {
                    let y = r.top() + e / 4.0 + i as f32 * (e / 2.0) / 2.0;
                    p.line_segment(
                        [egui::pos2(r.left() + e / 8.0, y), egui::pos2(r.right() - e / 8.0, y)],
                        st,
                    );
                }
            },
            Action::AddModule => {
                p.line_segment([egui::pos2(c.x, r.top()), egui::pos2(c.x, r.bottom())], st);
                p.line_segment([egui::pos2(r.left(), c.y), egui::pos2(r.right(), c.y)], st);
            },
            _ => {},
        }
    }

    /// A rail button as glyph + permanent micro-label (the operator's rail decision): the
    /// mockup's 44 px box and 52 px pitch, the board's word. Registration is the same as
    /// [`Self::button`] — one audit list for everything you can touch.
    #[allow(clippy::too_many_arguments)]
    fn glyph_button(
        &mut self,
        p: &Painter,
        pal: &Palette,
        id: &'static str,
        rect: Rect,
        action: Action,
        accent: Option<Color32>,
    ) {
        let eg = egui_rect(rect);
        p.rect_filled(eg, LAYOUT_CORNER_MICRO as u8, pal.ground_panel_alt);
        p.rect_stroke(
            eg,
            LAYOUT_CORNER_MICRO as u8,
            pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32),
            egui::StrokeKind::Middle,
        );
        let color = accent.unwrap_or(pal.text_secondary);
        let label = match action {
            Action::Play => "PLAY",
            Action::Stop => "STOP",
            Action::Panic => "PANIC",
            Action::ToggleMode => "MODE",
            Action::ToggleDock => "MODS",
            Action::ToggleInspector => "DIAG",
            Action::AddModule => "ADD",
            _ => "",
        };
        Self::glyph(p, eg.center() - egui::vec2(0.0, LAYOUT_SPACE_1 as f32), action, color);
        p.text(
            eg.center() + egui::vec2(0.0, LAYOUT_SPACE_3 as f32),
            Align2::CENTER_CENTER,
            label,
            font_xs(),
            color,
        );
        self.registry.push(Registered { id: id.to_string(), rect, class: TouchClass::S, action });
        self.audit_elements.push(InteractiveElement {
            id: id.to_string(),
            class: TouchClass::S,
            rect,
            dense_allowed: false,
        });
    }

    fn draw_rail(&mut self, p: &Painter, rail: Rect, pal: &Palette) {
        let eg = egui_rect(rail);
        p.rect_filled(eg, LAYOUT_CORNER_NONE as u8, pal.ground_panel);
        p.line_segment(
            [eg.right_top(), eg.right_bottom()],
            pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32),
        );
        // 44 px buttons in a 56 px rail: the token pair (rail_width, rail_button) exists exactly
        // for this. The mockup's grouped sections (design-mode.svg): transport, view, and at the
        // foot the level tick + the ADD cross; hairline dividers between groups, as drawn there.
        let b = LAYOUT_SHELL_RAIL_BUTTON as f32;
        let gap = LAYOUT_SPACE_2 as f32;
        let x0 = rail.min.x + (rail.width() - b) / 2.0;
        let mut y = rail.min.y + gap;
        let groups: [[(&str, Action); 3]; 2] = [
            [
                ("rail/transport/play", Action::Play),
                ("rail/transport/stop", Action::Stop),
                ("rail/transport/panic", Action::Panic),
            ],
            [
                ("rail/mode/toggle", Action::ToggleMode),
                ("rail/browser", Action::ToggleDock),
                ("rail/diagnostics", Action::ToggleInspector),
            ],
        ];
        for (gi, group) in groups.iter().enumerate() {
            if gi > 0 {
                y += gap;
                p.line_segment(
                    [egui::pos2(rail.min.x + gap, y), egui::pos2(rail.max.x - gap, y)],
                    pal.hairline(pal.hairline_faint, LAYOUT_STROKE_HAIRLINE as f32),
                );
                y += gap;
            }
            for (id, action) in group {
                let rect = Rect::new(Vec2::new(x0, y), Vec2::new(x0 + b, y + b));
                // Panic is red before you press it: the one control whose colour carries urgency
                // rather than signal class — and it stays redundant with the word PANIC, because
                // nothing may be encoded by colour alone (look-board rule).
                let accent = match action {
                    Action::Panic => Some(pal.error),
                    Action::Play => Some(pal.playing),
                    _ => None,
                };
                self.glyph_button(p, pal, id, rect, *action, accent);
                y += b + gap;
            }
        }
        // The foot: the live level tick (the mockup's green well, in the AUDIO class colour —
        // §4: colour is the signal class, so a master-audio meter is amber, not green) and the
        // ADD cross that opens the browser at the canvas centre.
        let foot =
            Rect::new(Vec2::new(x0, rail.max.y - gap - b), Vec2::new(x0 + b, rail.max.y - gap));
        let tick_h = LAYOUT_SPACE_1 as f32;
        let tick_w = LAYOUT_SPACE_4 as f32 * 2.0 + tick_h;
        let tick = Rect::new(
            Vec2::new(x0 + (b - tick_w) / 2.0, foot.min.y - gap - tick_h),
            Vec2::new(x0 + (b + tick_w) / 2.0, foot.min.y - gap),
        );
        let te = egui_rect(tick);
        p.rect_filled(te, LAYOUT_CORNER_NONE as u8, pal.ground_inset);
        let level = self
            .canvas
            .resolve_master(&self.graph)
            .map(|m| self.canvas.levels.get(m))
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        if level > 0.0 {
            let fill = egui::Rect::from_min_max(
                te.left_top(),
                egui::pos2(te.left() + te.width() * level, te.bottom()),
            );
            p.rect_filled(fill, LAYOUT_CORNER_NONE as u8, pal.audio);
        }
        self.glyph_button(p, pal, "rail/add", foot, Action::AddModule, Some(pal.event));
    }

    fn draw_canvas(&mut self, p: &Painter, layout: &ShellLayout, pal: &Palette) {
        let eg = egui_rect(layout.canvas);
        p.rect_filled(eg, LAYOUT_CORNER_NONE as u8, pal.ground_canvas);

        if self.state.mode == ShellMode::Design {
            // The graph, drawn by the canvas painter from the computed layout (WO-013). The
            // painter registers node/port/menu touch targets into the same audit list the chrome
            // uses, so the canvas is measured by the same gate as everything else.
            let ctx = ConnectContext::no_adapters(Phase::Zero);
            // The scope traces (WO-013 increment 6, D3′): the LIVE session is the single
            // source; at rest the painter gets an empty set and every scope shows its rest
            // line — a dead stream's signal is never left on screen.
            let no_traces = sparq_ui::canvas::scope::ScopeTraces::new();
            let traces = self.live.as_ref().map(|s| s.traces()).unwrap_or(&no_traces);
            let no_meters = sparq_ui::canvas::levels::LiveMeters::new();
            let meters = self.live.as_ref().map(|s| s.meters()).unwrap_or(&no_meters);
            canvas_ui::draw(
                p,
                pal,
                &self.graph,
                &self.canvas,
                &self.canvas_layout,
                layout.canvas,
                &ctx,
                traces,
                meters,
                &mut self.audit_elements,
            );
            // The wire-encoding legend (increment 3, the mockup's floating box): top-right of
            // the canvas, Design mode, every LOD but Dot — at Dot the canvas is dots and
            // hairlines by contract, chrome included. Drawn from the wires' own encoding table.
            if self.canvas_layout.lod != Lod::Dot {
                let lw = LAYOUT_SPACE_9 as f32 + LAYOUT_SPACE_7 as f32;
                let lh = LAYOUT_SPACE_4 as f32 * 5.0 + LAYOUT_SPACE_2 as f32 * 2.0;
                let lx = layout.canvas.max.x - lw - LAYOUT_SPACE_4 as f32;
                let ly = layout.canvas.min.y + LAYOUT_SPACE_4 as f32;
                canvas_ui::draw_wire_legend(
                    p,
                    pal,
                    Rect::new(Vec2::new(lx, ly), Vec2::new(lx + lw, ly + lh)),
                );
            }
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
            // The port summary (increment 3): one dot per port in manifest order — INPUTS as
            // rings, OUTPUTS filled (direction never rides colour alone), each in its signal
            // class. The node's ports at a glance, from the spec: data, not decoration.
            let mut dx = il.title.min.x;
            let dy = il.title.max.y + (LAYOUT_SPACE_4 as f32) / 2.0;
            let pr = (LAYOUT_SPACE_2 as f32) / 2.0;
            for pt in &node.spec.ports {
                let col = canvas_ui::class_colour(sparq_ui::canvas::layout::signal_class(pt), pal);
                let c = egui::pos2(dx + pr, dy);
                if pt.direction == sparq_module_api::port::Direction::Out {
                    p.circle_filled(c, pr, col);
                } else {
                    p.circle_stroke(c, pr, Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, col));
                }
                dx += LAYOUT_SPACE_4 as f32;
            }
            if il.rows.is_empty() {
                p.text(
                    egui::pos2(il.title.min.x, il.title.max.y + LAYOUT_SPACE_4 as f32),
                    Align2::LEFT_TOP,
                    "this module declares no parameters",
                    font_s(),
                    pal.text_disabled,
                );
            }
            for row in il.visible_rows() {
                let Some(desc) = node.spec.params.get(row.index) else { continue };
                // `visible_rows` is the layout's own clip (rows under the fixed header or below
                // the panel bottom are skipped) — and `row_at` refuses exactly the same rows, so
                // the clip is honest in both directions: what you cannot see you cannot touch.
                let value = node.param_value(row.index).unwrap_or(desc.default as f32);
                p.text(
                    egui::pos2(row.label.min.x, row.label.center().y),
                    Align2::LEFT_CENTER,
                    desc.name.as_str(),
                    font_s(),
                    if row.editable { pal.text_secondary } else { pal.text_disabled },
                );
                if row.editable {
                    // The mockup's slider language (increment 3): a CONTROL-class track —
                    // cyan, because a slider is a control, §4 — with the value's fill at signal
                    // weight and a vertical BLOCK thumb (the mockup's), not a circle: a bar
                    // reads as a position on a scale at a glance, a dot reads as a datapoint.
                    let mid_y = row.track.center().y;
                    let (a, b) =
                        (egui::pos2(row.track.min.x, mid_y), egui::pos2(row.track.max.x, mid_y));
                    p.line_segment([a, b], Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, pal.cv));
                    let kx = sparq_ui::canvas::inspector::knob_x(desc, row.track, value);
                    let knob = egui::pos2(kx, mid_y);
                    p.line_segment([a, knob], Stroke::new(LAYOUT_STROKE_SIGNAL as f32, pal.cv));
                    let thumb = egui::Rect::from_center_size(
                        knob,
                        egui::vec2(LAYOUT_SPACE_2 as f32, LAYOUT_SPACE_5 as f32),
                    );
                    p.rect_filled(thumb, LAYOUT_CORNER_MICRO as u8, pal.cv);
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
            // The scrollbar thumb (increment 5): drawn only when the panel actually scrolls — a
            // scrollbar that cannot move is chrome pretending to be a control. It is an
            // indicator, not a v0 touch target (the two-finger pan is the gesture), so it is
            // deliberately NOT registered in the audit.
            if let Some(thumb) = il.thumb() {
                p.rect_filled(
                    egui_rect(thumb),
                    LAYOUT_CORNER_NONE as u8,
                    pal.hairline_colour.gamma_multiply(pal.hairline_regular),
                );
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

        // Tabs: MODULES is LIVE — the card grid below is its content. The other four stay
        // DISABLED words until their subsystems exist; drawing them interactive with nothing
        // behind them would be the lie the stub exists to avoid.
        let char_w = LAYOUT_SPACE_2 as f32;
        let mut x = dock.min.x + LAYOUT_SPACE_PADDING_PANEL as f32;
        for (tab, live) in [
            ("MODULES", true),
            ("LIBRARY", false),
            ("STREAMS", false),
            ("SCENES", false),
            ("JOURNAL", false),
        ] {
            p.text(
                egui::pos2(x, dock.min.y + hb / 2.0),
                Align2::LEFT_CENTER,
                tab,
                font_xs(),
                if live { pal.text_primary } else { pal.text_disabled },
            );
            if live {
                p.line_segment(
                    [
                        egui::pos2(x, dock.min.y + hb - LAYOUT_STROKE_HAIRLINE as f32),
                        egui::pos2(
                            x + tab.len() as f32 * char_w,
                            dock.min.y + hb - LAYOUT_STROKE_HAIRLINE as f32,
                        ),
                    ],
                    Stroke::new(LAYOUT_STROKE_SIGNAL as f32, pal.audio),
                );
            }
            x += tab.len() as f32 * char_w + LAYOUT_SPACE_5 as f32;
        }

        // The module palette (increment 4, the operator's ask): SMALL tiles (112x48, class M —
        // the 44 px floor holds), grouped by the manifest's top category in registry order,
        // each group under its xs word; the tile's left stripe is its dominant SIGNAL class
        // (§4: colour says what the module CARRIES, the group says what it IS — never
        // accent-by-category). A tap spawns at the canvas centre through the browser's own op
        // path: undoable, ledger-marked, one undo step.
        let cw = LAYOUT_SHELL_DOCK_CARD_W as f32;
        let ch = LAYOUT_SHELL_DOCK_CARD_H as f32;
        let gap = LAYOUT_SPACE_2 as f32;
        let pad = LAYOUT_SPACE_PADDING_PANEL as f32;
        let body_top = dock.min.y + hb + gap * 2.0;
        let head_h = LAYOUT_SPACE_4 as f32;
        // Group catalogue indices by top category, preserving registry order.
        let cat = self.canvas.catalog();
        let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
        for (i, item) in cat.iter().enumerate() {
            let top = item.spec.module_id.split('/').next().unwrap_or("?");
            match groups.iter_mut().find(|(g, _)| *g == top) {
                Some((_, v)) => v.push(i),
                None => groups.push((top, vec![i])),
            }
        }
        let rows_fit = (((dock.height() - hb - gap * 3.0) / (ch + gap)).floor() as usize).max(1);
        let mut x = dock.min.x + pad;
        let mut shown = 0usize;
        for (name, members) in &groups {
            let cols_needed = members.len().div_ceil(rows_fit);
            let group_w = cols_needed as f32 * (cw + gap) - gap;
            if x + group_w > dock.max.x - pad {
                break; // what does not fit is counted in words below — never drawn half
            }
            p.text(
                egui::pos2(x, body_top - gap),
                Align2::LEFT_CENTER,
                name.to_uppercase(),
                font_xs(),
                pal.text_tertiary,
            );
            for (k, &idx) in members.iter().enumerate() {
                let (c, r) = (k / rows_fit, k % rows_fit);
                let item = &cat[idx];
                let rect = Rect::new(
                    Vec2::new(x + c as f32 * (cw + gap), body_top + head_h + r as f32 * (ch + gap)),
                    Vec2::new(
                        x + c as f32 * (cw + gap) + cw,
                        body_top + head_h + r as f32 * (ch + gap) + ch,
                    ),
                );
                let eg = egui_rect(rect);
                p.rect_filled(eg, LAYOUT_CORNER_MICRO as u8, pal.ground_panel_alt);
                p.rect_stroke(
                    eg,
                    LAYOUT_CORNER_MICRO as u8,
                    pal.hairline(pal.hairline_faint, LAYOUT_STROKE_HAIRLINE as f32),
                    egui::StrokeKind::Middle,
                );
                // The stripe: dominant signal class, signal weight (2 px) — what the module
                // carries, in the class language, on the tile's left edge.
                use sparq_module_api::port::Direction;
                let cls = item
                    .spec
                    .ports
                    .iter()
                    .find(|pt| pt.direction == Direction::Out)
                    .or_else(|| item.spec.ports.iter().find(|pt| pt.direction == Direction::In))
                    .map(sparq_ui::canvas::layout::signal_class)
                    .unwrap_or(sparq_ui::canvas::layout::SignalClass::Neutral);
                let stripe = egui::Rect::from_min_size(
                    eg.left_top(),
                    egui::vec2(LAYOUT_STROKE_SIGNAL as f32, eg.height()),
                );
                p.rect_filled(stripe, LAYOUT_CORNER_NONE as u8, canvas_ui::class_colour(cls, pal));
                p.text(
                    egui::pos2(eg.min.x + LAYOUT_SPACE_3 as f32, eg.center().y),
                    Align2::LEFT_CENTER,
                    item.spec.display_name.as_str(),
                    font_xs(),
                    pal.text_secondary,
                );
                let id: String = format!("dock/card/{idx}");
                self.registry.push(Registered {
                    id: id.clone(),
                    rect,
                    class: TouchClass::M,
                    action: Action::SpawnFromDock(idx),
                });
                self.audit_elements.push(InteractiveElement {
                    id,
                    class: TouchClass::M,
                    rect,
                    dense_allowed: false,
                });
                shown += 1;
            }
            x += group_w + LAYOUT_SPACE_5 as f32;
        }
        let total = self.canvas.catalog_len();
        if total > shown {
            p.text(
                egui::pos2(dock.max.x - pad, dock.max.y - LAYOUT_SPACE_4 as f32),
                Align2::RIGHT_CENTER,
                format!("+ {} more below — resize the dock, or search with MODS", total - shown),
                font_xs(),
                pal.text_tertiary,
            );
        }
    }
}
