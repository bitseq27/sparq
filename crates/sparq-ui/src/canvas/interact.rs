//! The canvas interaction state: gestures in, operations and *explained* refusals out.
//!
//! This is the WO-013 half of the input model's rule — *a gesture produces an intent, not an
//! effect*. The WO-012 recogniser turns pointers into [`GestureIntent`]s; this file turns the ones
//! aimed at the canvas into [`Op`]s on the [`Graph`], through the connection verdicts in
//! [`super::connect`]. It owns the camera, the selection, the in-flight interaction (move / wire /
//! marquee), the long-press context menu, and the undo history.
//!
//! Every refusal is returned as a [`CanvasEvent`] with a sentence, because "my touch did nothing"
//! must never be a mystery on stage (input-model §3). The mapping is tabulated in
//! `docs/ui/gestures.md`.

use std::collections::BTreeSet;

use crate::canvas::browser::{BrowserHit, BrowserItem, BrowserState};
use crate::canvas::camera::{Camera, Lod};
use crate::canvas::connect::{self, ConnectContext, ConnectOutcome};
use crate::canvas::inspector::{self, InspectorLayout};
use crate::canvas::layout::{self, CanvasLayout, Hit, WireEndSide};
use crate::canvas::model::{Graph, NodeId, Op, PortRef, UndoStack, Wire, WireId};
use crate::geom::{Rect, Vec2};
use crate::gesture::GestureIntent;
use crate::tokens::{
    LAYOUT_CANVAS_SNAP, LAYOUT_SPACE_2, LAYOUT_SPACE_3, LAYOUT_SPACE_PADDING_SECTION,
    LAYOUT_TOUCH_ROW_HEIGHT_LIST,
};
use sparq_module_api::port::Direction;

/// Which nodes and wires are selected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Selected nodes.
    pub nodes: BTreeSet<NodeId>,
    /// Selected wires.
    pub wires: BTreeSet<WireId>,
}

impl Selection {
    /// Nothing selected.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
    /// Whether nothing is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.wires.is_empty()
    }
    /// Total selected count (nodes + wires), for logging.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len() + self.wires.len()
    }
    /// Clear the selection.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.wires.clear();
    }
}

/// An in-flight wire being dragged from a port.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingWire {
    /// The port the drag started from.
    pub from: PortRef,
    /// Its direction (a drag may start from an input; the wire is normalised out→in on drop).
    pub from_dir: Direction,
    /// Where the finger is now, in screen px.
    pub cursor_screen: Vec2,
    /// The port the cursor is currently capturing (magnet), if any.
    pub hovered: Option<PortRef>,
}

/// What a drag is doing right now.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Interaction {
    /// Nothing in flight.
    #[default]
    Idle,
    /// Moving a set of nodes; `orig` is their pre-drag world positions, `acc_screen` the
    /// accumulated (fine-scaled) screen delta.
    Move {
        /// Pre-drag positions.
        orig: Vec<(NodeId, Vec2)>,
        /// Accumulated screen delta (already fine-scaled).
        acc_screen: Vec2,
    },
    /// Dragging a wire out of a port.
    Wire(PendingWire),
    /// Re-patching one end of an EXISTING wire (increment 3): the other end stays put; `orig` is
    /// the wire as it was, so a refusal or a drop on empty canvas restores it exactly.
    Repatch {
        /// The wire being re-patched.
        wire: WireId,
        /// Which end is detached and following the finger.
        side: WireEndSide,
        /// The wire as it was before the drag (restore data).
        orig: Wire,
        /// Where the finger is now, screen px.
        cursor_screen: Vec2,
        /// The port the cursor is currently capturing (magnet), if any.
        hovered: Option<PortRef>,
    },
    /// Dragging an inspector slider (increment 3). `pushed` records whether this drag has its own
    /// history entry yet — the first value change pushes one, later changes within the SAME drag
    /// replace it, so one gesture is one undo step (and two gestures never merge).
    Param {
        /// The node being edited.
        node: NodeId,
        /// Index into the node spec's params.
        index: usize,
        /// Where the finger is now, screen px.
        cursor_screen: Vec2,
        /// Whether this drag already pushed its history entry.
        pushed: bool,
    },
    /// Rubber-band multi-select; screen anchor + accumulated screen delta.
    Marquee {
        /// Where the drag began, screen px.
        start_screen: Vec2,
        /// Accumulated screen delta.
        acc_screen: Vec2,
    },
}

/// An action a long-press context menu row performs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Copy the node (offset onto the grid), select the copy.
    Duplicate,
    /// Toggle the bypass flag.
    Bypass,
    /// Toggle the mute flag.
    Mute,
    /// Toggle the lock flag.
    Lock,
    /// Delete the node (with its wires) or the wire.
    Delete,
    /// Select every node.
    SelectAll,
    /// Zoom to fit the graph.
    ZoomFit,
    /// Redo the last undone op.
    Redo,
    /// Make this node the master — the one the listener hears (bridge renders it).
    SetMaster,
    /// Render the patch to a WAV through the executor (the shell executes; the canvas asks).
    RenderWav,
    /// Open the module browser at the long-press position (increment 3).
    OpenBrowser,
}

/// One row of the context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuRow {
    /// What it does.
    pub action: MenuAction,
    /// The label shown (uppercase, terse — the token type language).
    pub label: &'static str,
    /// Whether it can be chosen (a locked node's Delete is shown disabled, never silent).
    pub enabled: bool,
}

impl MenuRow {
    fn row(action: MenuAction, label: &'static str, enabled: bool) -> Self {
        Self { action, label, enabled }
    }
}

/// What the context menu is attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuTarget {
    /// A node.
    Node(NodeId),
    /// A wire.
    Wire(WireId),
    /// Empty canvas.
    Empty,
}

/// An open context menu.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuState {
    /// What it targets.
    pub target: MenuTarget,
    /// Where the long-press landed, screen px (the menu anchors here, clamped into view).
    pub anchor: Vec2,
    /// The rows.
    pub rows: Vec<MenuRow>,
}

impl MenuState {
    /// The menu width in screen px, from the longest label (monospace advance on the 8 px space
    /// scale, the same budget the shell's top bar uses — no font metrics hard-coded).
    #[must_use]
    pub fn width(&self) -> f32 {
        let longest = self.rows.iter().map(|r| r.label.len()).max().unwrap_or(0);
        let char_w = LAYOUT_SPACE_2 as f32;
        let pad = LAYOUT_SPACE_3 as f32 * 2.0;
        pad + longest as f32 * char_w
    }

    /// The menu height in screen px.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.rows.len() as f32 * LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32
    }

    /// The rect of row `i`, given the clamped top-left `origin` (screen px). The painter and the
    /// hit-test both call this, so the row you see is the row you touch.
    #[must_use]
    pub fn row_rect(&self, origin: Vec2, i: usize) -> Rect {
        Rect::from_min_size(
            Vec2::new(origin.x, origin.y + i as f32 * LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32),
            Vec2::new(self.width(), LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32),
        )
    }
}

/// Keep a menu of `size` inside `view` given a requested `anchor`. Shared by the context menu and
/// the module browser sheet — one clamping rule, so both overlays behave the same near edges.
pub(super) fn clamp_origin(anchor: Vec2, size: Vec2, view: Rect) -> Vec2 {
    let x = anchor.x.min(view.max.x - size.x).max(view.min.x);
    let y = anchor.y.min(view.max.y - size.y).max(view.min.y);
    Vec2::new(x, y)
}

/// Something the canvas did, for the shell's "explain yourself" log.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEvent {
    /// An op was applied; carries its label.
    Applied(String),
    /// An action was refused; carries the sentence shown to the user.
    Refused(String),
    /// Informational (a replacement happened, a conversion was drawn, a drag was dropped).
    Note(String),
    /// The selection changed; carries the new count.
    Selection(usize),
    /// Undo ran; carries the undone op's label.
    Undo(String),
    /// Redo ran; carries the redone op's label.
    Redo(String),
    /// The menu opened (`true`) or closed (`false`).
    Menu(bool),
    /// Zoom-to-fit ran.
    ZoomToFit,
    /// The level of detail changed.
    Lod(Lod),
    /// The master node changed; carries the new master.
    MasterSet(crate::canvas::model::NodeId),
    /// The user asked for a render. The canvas cannot render (it knows no executor); the shell
    /// consumes this event and does it — an intent, not an effect, all the way down.
    RenderWav,
    /// The module browser opened (`true`) or closed (`false`).
    Browser(bool),
    /// The browser's query changed; carries the match count (the shell's log shows the ranking
    /// is alive while the user types).
    BrowserQuery(usize),
}

impl CanvasEvent {
    /// The log line for this event.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Applied(s) => format!("canvas: {s}"),
            Self::Refused(s) => format!("canvas refused: {s}"),
            Self::Note(s) => format!("canvas: {s}"),
            Self::Selection(n) => format!("canvas: {n} selected"),
            Self::Undo(s) => format!("canvas undo: {s}"),
            Self::Redo(s) => format!("canvas redo: {s}"),
            Self::Menu(true) => "canvas: context menu".to_string(),
            Self::Menu(false) => "canvas: menu closed".to_string(),
            Self::ZoomToFit => "canvas: zoom to fit".to_string(),
            Self::Lod(l) => format!("canvas: LOD {l:?}"),
            Self::MasterSet(id) => format!("canvas: master = node {id}"),
            Self::RenderWav => "canvas: RENDER WAV requested".to_string(),
            Self::Browser(true) => "canvas: module browser open".to_string(),
            Self::Browser(false) => "canvas: module browser closed".to_string(),
            Self::BrowserQuery(n) => format!("canvas: browser query — {n} match(es)"),
        }
    }
}

/// All canvas state carried between frames.
#[derive(Clone, Debug)]
pub struct CanvasState {
    /// The viewport.
    pub camera: Camera,
    /// What is selected.
    pub selection: Selection,
    /// The in-flight drag, if any.
    pub interaction: Interaction,
    /// The open context menu, if any.
    pub menu: Option<MenuState>,
    /// Undo/redo history.
    pub history: UndoStack,
    /// The explicit master node (long-press → SET MASTER). `None` = use [`Self::resolve_master`]'s
    /// documented default rule. Until `out/main` exists (WO-014), this is what tells the bridge
    /// which node feeds the listener.
    pub master: Option<crate::canvas::model::NodeId>,
    /// The open module browser, if any (increment 3). Modal over the canvas like the menu.
    pub browser: Option<BrowserState>,
    /// The module catalogue the browser ranks: what the registry actually has, supplied by the
    /// shell at startup — the canvas never invents modules (defect #58, structurally).
    catalog: Vec<BrowserItem>,
    /// The inspector panel geometry for the current selection, recomputed by the shell each
    /// frame (`None` when nothing is selected or the panel is collapsed). The canvas reads it to
    /// route slider drags; the painter reads it to draw.
    inspector: Option<InspectorLayout>,
    /// Per-node signal levels for the live wire-level animation (WO-013 increment 4). TRANSIENT
    /// and NOT part of the undoable model: a level is a fact about the last render, not an edit.
    /// The shell refreshes it from the executor's meters (the bridge reads them); the painter maps
    /// it onto wires through [`crate::canvas::levels::wire_level`]. Empty until the first render,
    /// so wires draw at rest exactly as they did before this increment — no faked levels.
    pub levels: crate::canvas::levels::NodeLevels,
    last_lod: Lod,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasState {
    /// A fresh canvas: default camera, empty selection, no history.
    #[must_use]
    pub fn new() -> Self {
        let cam = Camera::new();
        Self {
            last_lod: cam.lod(),
            camera: cam,
            selection: Selection::empty(),
            interaction: Interaction::Idle,
            menu: None,
            history: UndoStack::new(),
            master: None,
            browser: None,
            catalog: Vec::new(),
            inspector: None,
            levels: crate::canvas::levels::NodeLevels::new(),
        }
    }

    /// Supply the browser's catalogue (the shell builds it from the registry: one [`BrowserItem`]
    /// per installed module, spec + summary + category straight from the validated manifest).
    pub fn set_catalog(&mut self, items: Vec<BrowserItem>) {
        self.catalog = items;
    }

    /// The catalogue size (diagnostics, tests).
    #[must_use]
    pub fn catalog_len(&self) -> usize {
        self.catalog.len()
    }

    /// The open browser, for the painter.
    #[must_use]
    pub fn browser(&self) -> Option<&BrowserState> {
        self.browser.as_ref()
    }

    /// Replace the browser's query (the shell pipes its text entry here) and report the match
    /// count as an event, so the log shows the ranking responding.
    pub fn browser_set_query(&mut self, q: &str) -> Vec<CanvasEvent> {
        match &mut self.browser {
            Some(b) => {
                b.set_query(q);
                vec![CanvasEvent::BrowserQuery(b.visible_len())]
            },
            None => Vec::new(),
        }
    }

    /// Move the browser's selection (a scroll/arrow from the shell), clamped by the core.
    pub fn browser_move_selection(&mut self, delta: i64, page_rows: usize) {
        if let Some(b) = &mut self.browser {
            b.move_selection(delta, page_rows);
        }
    }

    /// Navigate the browser rows by keyboard/scroll: the shell pipes arrows, the core owns the
    /// page geometry (one copy of the row math, shared with the hit-test).
    pub fn browser_navigate(&mut self, delta: i64, view: Rect) {
        if let Some(b) = &mut self.browser {
            let (_, max_h) = crate::canvas::browser::caps(view);
            b.move_selection(delta, BrowserState::page_rows(max_h));
        }
    }

    /// Confirm the browser's selection (the shell's Enter path): spawn the selected module,
    /// close the sheet. Same code path as a row tap — one spawn rule, two inputs.
    pub fn browser_confirm(&mut self, graph: &mut Graph) -> Vec<CanvasEvent> {
        if self.browser.is_some() {
            self.spawn_selected(graph)
        } else {
            Vec::new()
        }
    }

    /// Close the browser without spawning (the shell's Escape path; a tap outside closes via
    /// [`Self::on_intent`]'s activate path instead).
    pub fn browser_close(&mut self) -> Vec<CanvasEvent> {
        if self.browser.take().is_some() {
            vec![CanvasEvent::Browser(false)]
        } else {
            Vec::new()
        }
    }

    /// Store this frame's inspector geometry (the shell computes it from the selection and the
    /// shell layout's inspector rect).
    pub fn set_inspector(&mut self, il: Option<InspectorLayout>) {
        self.inspector = il;
    }

    /// The inspector geometry, for the painter.
    #[must_use]
    pub fn inspector(&self) -> Option<&InspectorLayout> {
        self.inspector.as_ref()
    }

    /// Which node feeds the listener: the explicit master when set (and still present), else —
    /// the WO-014 increment-5 handover — **an `out/main` node in the patch**, else the default
    /// rule (the highest-id node with an audio output AND at least one wire, preferring a
    /// terminus). The `out/main` step exists so the MASTER badge never lies: when a patch has a
    /// real output module, THAT is the master by name, not by a guess about which gain node
    /// happens to sit last. The rule never guesses silently — every render logs the master it
    /// chose. Unconnected spare modules (no wires at all) are excluded everywhere: rendering one
    /// would produce silence and confusion.
    #[must_use]
    pub fn resolve_master(&self, graph: &Graph) -> Option<crate::canvas::model::NodeId> {
        use sparq_module_api::port::{Direction, PortType};
        if let Some(m) = self.master {
            if graph.node(m).is_some() {
                return Some(m);
            }
        }
        let wired = |id: crate::canvas::model::NodeId| {
            graph.wires().iter().any(|w| w.from.node == id || w.to.node == id)
        };
        // The handover: the highest-id WIRED `out/main` is the master (an unwired one would
        // render silence, so it does not count). Falls through when the patch has none.
        let out_main = graph
            .nodes()
            .iter()
            .filter(|n| n.spec.module_id == crate::canvas::OUT_MAIN_ID && wired(n.id))
            .map(|n| n.id)
            .max();
        if let Some(m) = out_main {
            return Some(m);
        }
        let has_audio_out = |n: &crate::canvas::model::Node| {
            n.spec
                .ports
                .iter()
                .any(|p| p.direction == Direction::Out && p.port_type == PortType::Audio)
        };
        let terminal =
            |id: crate::canvas::model::NodeId| !graph.wires().iter().any(|w| w.from.node == id);
        let candidates: Vec<crate::canvas::model::NodeId> = graph
            .nodes()
            .iter()
            .filter(|n| has_audio_out(n) && wired(n.id))
            .map(|n| n.id)
            .collect();
        candidates
            .iter()
            .filter(|id| terminal(**id))
            .max()
            .or_else(|| candidates.iter().max())
            .copied()
    }

    /// The pending wire, if a wire drag is in flight (the painter reads this).
    #[must_use]
    pub fn pending_wire(&self) -> Option<&PendingWire> {
        match &self.interaction {
            Interaction::Wire(p) => Some(p),
            _ => None,
        }
    }

    /// The marquee rect in screen px, if a marquee is in flight.
    #[must_use]
    pub fn marquee_screen(&self) -> Option<Rect> {
        match &self.interaction {
            Interaction::Marquee { start_screen, acc_screen } => {
                let b = Vec2::new(start_screen.x + acc_screen.x, start_screen.y + acc_screen.y);
                Some(Rect::new(
                    Vec2::new(start_screen.x.min(b.x), start_screen.y.min(b.y)),
                    Vec2::new(start_screen.x.max(b.x), start_screen.y.max(b.y)),
                ))
            },
            _ => None,
        }
    }

    /// The clamped top-left of the open menu, given the view.
    #[must_use]
    pub fn menu_origin(&self, view: Rect) -> Option<Vec2> {
        self.menu.as_ref().map(|m| clamp_origin(m.anchor, Vec2::new(m.width(), m.height()), view))
    }

    /// Feed one intent aimed at the canvas. `layout` is the *previous* frame's (immediate mode:
    /// that is what was on screen when the finger came down); `view` is the canvas rect.
    pub fn on_intent(
        &mut self,
        graph: &mut Graph,
        intent: GestureIntent,
        layout: &CanvasLayout,
        view: Rect,
        ctx: &ConnectContext<'_>,
    ) -> Vec<CanvasEvent> {
        match intent {
            GestureIntent::Pan { delta } => {
                self.camera.pan_by_screen(delta);
                self.lod_events()
            },
            GestureIntent::Zoom { factor, center } => {
                self.camera.zoom_about(factor, center, view);
                self.lod_events()
            },
            GestureIntent::Undo => self.undo(graph),
            GestureIntent::DoubleTap { .. } => self.zoom_to_fit(graph, view),
            GestureIntent::Activate { pos } => self.activate(graph, pos, layout, view),
            GestureIntent::Context { pos } => self.open_menu(graph, pos, layout),
            GestureIntent::DragStart { pos } => self.drag_start(graph, pos, layout),
            GestureIntent::DragUpdate { delta, scale } => {
                self.drag_update(graph, delta, scale, layout)
            },
            GestureIntent::DragEnd { pos, cancelled } => {
                self.drag_end(graph, pos, cancelled, layout, view, ctx)
            },
            // Not canvas concerns: rotate is reserved for the rig map, flick for Perform scenes,
            // fine-changed is carried in DragUpdate's scale, and the panel/global intents belong to
            // the shell. The canvas declines them silently (the shell logs its own).
            GestureIntent::Rotate { .. }
            | GestureIntent::Flick { .. }
            | GestureIntent::DragFineChanged { .. }
            | GestureIntent::TogglePanel { .. }
            | GestureIntent::AllSoundOff
            | GestureIntent::RecoveryMenu => Vec::new(),
        }
    }

    fn lod_events(&mut self) -> Vec<CanvasEvent> {
        let lod = self.camera.lod();
        if lod != self.last_lod {
            self.last_lod = lod;
            vec![CanvasEvent::Lod(lod)]
        } else {
            Vec::new()
        }
    }

    fn undo(&mut self, graph: &mut Graph) -> Vec<CanvasEvent> {
        match self.history.undo(graph) {
            Some(op) => vec![CanvasEvent::Undo(op.label().to_string())],
            None => vec![CanvasEvent::Refused("nothing to undo".to_string())],
        }
    }

    fn redo(&mut self, graph: &mut Graph) -> Vec<CanvasEvent> {
        match self.history.redo(graph) {
            Some(op) => vec![CanvasEvent::Redo(op.label().to_string())],
            None => vec![CanvasEvent::Refused("nothing to redo".to_string())],
        }
    }

    fn zoom_to_fit(&mut self, graph: &Graph, view: Rect) -> Vec<CanvasEvent> {
        let bounds = graph.world_bounds(|n| layout::node_size(&n.spec));
        self.camera.zoom_to_fit(bounds, view, LAYOUT_SPACE_PADDING_SECTION as f32);
        let mut ev = vec![CanvasEvent::ZoomToFit];
        ev.extend(self.lod_events());
        ev
    }

    fn activate(
        &mut self,
        graph: &mut Graph,
        pos: Vec2,
        layout: &CanvasLayout,
        view: Rect,
    ) -> Vec<CanvasEvent> {
        // An open browser captures the whole canvas, like the menu: a row spawns, the header
        // points at the text entry, anything else closes it.
        if self.browser.is_some() {
            return self.activate_browser(graph, pos, view);
        }

        // An open menu captures the tap: a row runs its action, anything else closes the menu.
        if let Some(menu) = self.menu.clone() {
            let origin = self.menu_origin(view).unwrap_or(menu.anchor);
            for (i, row) in menu.rows.iter().enumerate() {
                if menu.row_rect(origin, i).contains(pos) {
                    if row.enabled {
                        self.menu = None;
                        let mut ev = vec![CanvasEvent::Menu(false)];
                        ev.extend(self.run_menu_action(
                            graph,
                            menu.target,
                            row.action,
                            menu.anchor,
                            view,
                        ));
                        return ev;
                    }
                    return vec![CanvasEvent::Refused(format!(
                        "'{}' is not available here",
                        row.label
                    ))];
                }
            }
            self.menu = None;
            return vec![CanvasEvent::Menu(false)];
        }

        // An inspector slider under the finger sets the value where you tapped (tap-to-set;
        // a drag from here adjusts continuously).
        if let Some(ev) = self.activate_inspector(graph, pos) {
            return ev;
        }

        let lod = self.camera.lod();
        match layout::hit_test(layout, pos, lod) {
            Hit::Port(pref, _) => {
                self.selection.clear();
                self.selection.nodes.insert(pref.node);
                vec![
                    CanvasEvent::Selection(1),
                    CanvasEvent::Note(
                        "drag from a port to patch; tap selects the node".to_string(),
                    ),
                ]
            },
            Hit::Node(id) => {
                // Tapping a node already in a multi-selection keeps the group (so the next drag
                // moves them together); tapping anything else selects it alone.
                if !self.selection.nodes.contains(&id) || self.selection.nodes.len() == 1 {
                    self.selection.clear();
                    self.selection.nodes.insert(id);
                }
                vec![CanvasEvent::Selection(self.selection.len())]
            },
            Hit::Wire(id) | Hit::WireEnd(id, _) => {
                self.selection.clear();
                self.selection.wires.insert(id);
                vec![CanvasEvent::Selection(1)]
            },
            Hit::Empty => {
                self.selection.clear();
                vec![CanvasEvent::Selection(0)]
            },
        }
    }

    /// Tap routing while the module browser is open. Sheet geometry caps mirror the painter's
    /// (the browser sheet lives inside the canvas view, at most half of it wide/tall).
    fn activate_browser(&mut self, graph: &mut Graph, pos: Vec2, view: Rect) -> Vec<CanvasEvent> {
        let (max_w, max_h) = crate::canvas::browser::caps(view);
        let hit =
            self.browser.as_ref().map_or(BrowserHit::Outside, |b| b.hit(pos, view, max_w, max_h));
        match hit {
            BrowserHit::Row(i) => {
                if let Some(b) = &mut self.browser {
                    b.select(i);
                }
                self.spawn_selected(graph)
            },
            BrowserHit::Query => {
                vec![CanvasEvent::Note(
                    "the search field takes the keyboard — type to filter".to_string(),
                )]
            },
            BrowserHit::Outside => {
                self.browser = None;
                vec![CanvasEvent::Browser(false)]
            },
        }
    }

    /// Spawn the browser's selected module where the browser was opened. Closes the browser —
    /// one row tap is one module, then you are back on the canvas wiring it (the command-surface
    /// behaviour; "add another" is one long-press away). A no-match query keeps the browser open
    /// and says so: closing it would hide the reason the tap did nothing.
    fn spawn_selected(&mut self, graph: &mut Graph) -> Vec<CanvasEvent> {
        let Some(b) = self.browser.take() else {
            return vec![CanvasEvent::Refused("the browser is not open".to_string())];
        };
        let Some(item) = b.selected_item() else {
            self.browser = Some(b);
            return vec![CanvasEvent::Refused(
                "nothing matches — clear the search or close the browser".to_string(),
            )];
        };
        let spec = item.spec.clone();
        self.spawn_node(graph, spec, b.spawn_world)
    }

    /// Insert a node at `world` (snapped to the grid; cascading off an occupied spot so a second
    /// spawn never lands exactly under the first).
    fn spawn_node(
        &mut self,
        graph: &mut Graph,
        spec: crate::canvas::model::NodeSpec,
        world: Vec2,
    ) -> Vec<CanvasEvent> {
        let grid = LAYOUT_CANVAS_SNAP as f32;
        let mut pos = snap(world, grid);
        let mut guard = 0usize;
        while guard < 64
            && graph.nodes().iter().any(|n| {
                (n.pos.x - pos.x).abs() < f32::EPSILON && (n.pos.y - pos.y).abs() < f32::EPSILON
            })
        {
            pos = snap(Vec2::new(pos.x + grid * 3.0, pos.y + grid * 3.0), grid);
            guard += 1;
        }
        let op = graph.op_add_node(spec, pos);
        let (new_id, name) = match &op {
            Op::AddNode(n) => (n.id, n.title().to_string()),
            _ => return vec![CanvasEvent::Refused("spawn failed".to_string())],
        };
        self.history.push(op);
        self.selection.clear();
        self.selection.nodes.insert(new_id);
        vec![
            CanvasEvent::Browser(false),
            CanvasEvent::Applied(format!("added {name}")),
            CanvasEvent::Selection(1),
        ]
    }

    /// Tap on an inspector row: select the node and set the parameter at the tapped x. Returns
    /// `None` when the tap is not on the inspector (the caller continues to the canvas).
    fn activate_inspector(&mut self, graph: &mut Graph, pos: Vec2) -> Option<Vec<CanvasEvent>> {
        let (node_id, index, editable, track) = self.inspector.as_ref().and_then(|il| {
            il.row_at(pos)
                .map(|i| (il.node, il.rows[i].index, il.rows[i].editable, il.rows[i].track))
        })?;
        let desc = graph.node(node_id)?.spec.params.get(index)?.clone();
        self.selection.clear();
        self.selection.nodes.insert(node_id);
        if !editable {
            return Some(vec![CanvasEvent::Refused(format!(
                "`{}` ({:?}) is not editable in v0 — options/text editing arrives with manifest v1",
                desc.name, desc.kind
            ))]);
        }
        let v = inspector::value_from_x(&desc, track, pos.x);
        Some(self.apply_param(graph, node_id, index, v, false))
    }

    /// One parameter edit through the model (which clamps/snaps), with history. `coalesce` is
    /// true only for the updates *inside* an open slider drag whose first change already pushed:
    /// those replace the open entry, so one drag is one undo step and the log stays quiet until
    /// the finger lifts. Everything else pushes its own entry and reports the new value.
    fn apply_param(
        &mut self,
        graph: &mut Graph,
        node: NodeId,
        index: usize,
        value: f32,
        coalesce: bool,
    ) -> Vec<CanvasEvent> {
        let Some(op) = graph.op_set_param(node, index, value) else {
            return Vec::new(); // no change (or refused by the model): silent, the slider says it
        };
        if coalesce {
            if let Some(Op::SetParam { node: pn, index: pi, from: pfrom, .. }) = self.history.top()
            {
                if *pn == node && *pi == index {
                    // Merge into the open entry — keeping its ORIGINAL `from`, so one undo
                    // restores the value the finger found, not the drag's first waypoint.
                    if let Op::SetParam { to, .. } = &op {
                        self.history.replace_top(Op::SetParam {
                            node,
                            index,
                            from: *pfrom,
                            to: *to,
                        });
                    }
                    return Vec::new();
                }
            }
        }
        let text = match &op {
            Op::SetParam { to, .. } => graph
                .node(node)
                .and_then(|n| n.spec.params.get(index))
                .map(|d| format!("{} = {}", d.name, inspector::value_text(d, *to)))
                .unwrap_or_else(|| format!("param {index} = {to}")),
            _ => "param".to_string(),
        };
        self.history.push(op);
        vec![CanvasEvent::Applied(text)]
    }

    fn open_menu(&mut self, graph: &Graph, pos: Vec2, layout: &CanvasLayout) -> Vec<CanvasEvent> {
        // A long-press over an open browser closes it and opens the menu where you pressed —
        // the menu is the deeper modal (it can re-open the browser).
        let mut ev = Vec::new();
        if self.browser.is_some() {
            self.browser = None;
            ev.push(CanvasEvent::Browser(false));
        }
        let lod = self.camera.lod();
        let target = match layout::hit_test(layout, pos, lod) {
            Hit::Node(id) => {
                self.selection.clear();
                self.selection.nodes.insert(id);
                MenuTarget::Node(id)
            },
            Hit::Wire(id) | Hit::WireEnd(id, _) => {
                self.selection.clear();
                self.selection.wires.insert(id);
                MenuTarget::Wire(id)
            },
            Hit::Port(pref, _) => {
                self.selection.clear();
                self.selection.nodes.insert(pref.node);
                MenuTarget::Node(pref.node)
            },
            Hit::Empty => MenuTarget::Empty,
        };
        let rows: Vec<MenuRow> = match target {
            MenuTarget::Node(id) => {
                let flags = graph.node(id).map(|n| n.flags).unwrap_or_default();
                vec![
                    MenuRow::row(MenuAction::Duplicate, "DUPLICATE", true),
                    MenuRow::row(MenuAction::SetMaster, "SET MASTER", true),
                    MenuRow::row(
                        MenuAction::Bypass,
                        if flags.bypassed { "BYPASS ON" } else { "BYPASS OFF" },
                        true,
                    ),
                    MenuRow::row(
                        MenuAction::Mute,
                        if flags.muted { "MUTE ON" } else { "MUTE OFF" },
                        true,
                    ),
                    MenuRow::row(
                        MenuAction::Lock,
                        if flags.locked { "LOCK ON" } else { "LOCK OFF" },
                        true,
                    ),
                    // A locked node's Delete is shown disabled with the reason, never silent.
                    MenuRow::row(
                        MenuAction::Delete,
                        if flags.locked { "DELETE (LOCKED)" } else { "DELETE" },
                        !flags.locked,
                    ),
                ]
            },
            MenuTarget::Wire(_) => vec![MenuRow::row(MenuAction::Delete, "DELETE WIRE", true)],
            MenuTarget::Empty => vec![
                // ADD MODULE first: on an empty canvas it is the reason you long-pressed.
                // Disabled (with the reason) when the shell supplied no catalogue — never a row
                // that silently does nothing (#58's rule reaches the menu too).
                MenuRow::row(
                    MenuAction::OpenBrowser,
                    if self.catalog.is_empty() {
                        "ADD MODULE (NONE INSTALLED)"
                    } else {
                        "ADD MODULE"
                    },
                    !self.catalog.is_empty(),
                ),
                MenuRow::row(MenuAction::SelectAll, "SELECT ALL", true),
                MenuRow::row(MenuAction::ZoomFit, "ZOOM TO FIT", true),
                MenuRow::row(MenuAction::RenderWav, "RENDER WAV", true),
                MenuRow::row(MenuAction::Redo, "REDO", self.history.can_redo()),
            ],
        };
        self.menu = Some(MenuState { target, anchor: pos, rows });
        ev.push(CanvasEvent::Menu(true));
        ev
    }

    fn run_menu_action(
        &mut self,
        graph: &mut Graph,
        target: MenuTarget,
        action: MenuAction,
        anchor: Vec2,
        view: Rect,
    ) -> Vec<CanvasEvent> {
        match (target, action) {
            (MenuTarget::Node(id), MenuAction::Duplicate) => self.duplicate(graph, id),
            (MenuTarget::Node(id), MenuAction::SetMaster) => {
                self.master = Some(id);
                vec![CanvasEvent::MasterSet(id)]
            },
            (MenuTarget::Empty, MenuAction::RenderWav) => vec![CanvasEvent::RenderWav],
            (MenuTarget::Empty, MenuAction::OpenBrowser) => {
                // The sheet anchors at the press; the spawn point is the same press, un-projected
                // into the world — the module lands where you asked for it, on the grid.
                let spawn_world = self.camera.to_world(anchor, view);
                self.browser = Some(BrowserState::open(self.catalog.clone(), anchor, spawn_world));
                vec![CanvasEvent::Browser(true)]
            },
            (MenuTarget::Node(id), MenuAction::Bypass) => self.toggle_flag(graph, id, 0),
            (MenuTarget::Node(id), MenuAction::Mute) => self.toggle_flag(graph, id, 1),
            (MenuTarget::Node(id), MenuAction::Lock) => self.toggle_flag(graph, id, 2),
            (MenuTarget::Node(id), MenuAction::Delete) => self.delete_node(graph, id),
            (MenuTarget::Wire(id), MenuAction::Delete) => self.delete_wire(graph, id),
            (MenuTarget::Empty, MenuAction::SelectAll) => {
                self.selection.clear();
                for n in graph.nodes() {
                    self.selection.nodes.insert(n.id);
                }
                vec![CanvasEvent::Selection(self.selection.len())]
            },
            (MenuTarget::Empty, MenuAction::ZoomFit) => self.zoom_to_fit(graph, view),
            (MenuTarget::Empty, MenuAction::Redo) => self.redo(graph),
            _ => vec![CanvasEvent::Refused("that action does not apply here".to_string())],
        }
    }

    fn duplicate(&mut self, graph: &mut Graph, id: NodeId) -> Vec<CanvasEvent> {
        let Some(src) = graph.node(id).cloned() else {
            return vec![CanvasEvent::Refused("no such node".to_string())];
        };
        let grid = LAYOUT_CANVAS_SNAP as f32;
        let pos = snap(Vec2::new(src.pos.x + grid * 3.0, src.pos.y + grid * 3.0), grid);
        let op = graph.op_add_node(src.spec.clone(), pos);
        let new_id = match &op {
            Op::AddNode(n) => n.id,
            _ => return vec![CanvasEvent::Refused("duplicate failed".to_string())],
        };
        self.history.push(op);
        self.selection.clear();
        self.selection.nodes.insert(new_id);
        vec![CanvasEvent::Applied("duplicate".to_string()), CanvasEvent::Selection(1)]
    }

    fn toggle_flag(&mut self, graph: &mut Graph, id: NodeId, which: u8) -> Vec<CanvasEvent> {
        let Some(node) = graph.node(id) else {
            return vec![CanvasEvent::Refused("no such node".to_string())];
        };
        let mut to = node.flags;
        let label = match which {
            0 => {
                to.bypassed = !to.bypassed;
                if to.bypassed {
                    "bypass on"
                } else {
                    "bypass off"
                }
            },
            1 => {
                to.muted = !to.muted;
                if to.muted {
                    "mute on"
                } else {
                    "mute off"
                }
            },
            _ => {
                to.locked = !to.locked;
                if to.locked {
                    "lock on"
                } else {
                    "lock off"
                }
            },
        };
        match graph.op_set_flags(id, to) {
            Some(op) => {
                self.history.push(op);
                vec![CanvasEvent::Applied(label.to_string())]
            },
            None => vec![CanvasEvent::Note("no change".to_string())],
        }
    }

    fn delete_node(&mut self, graph: &mut Graph, id: NodeId) -> Vec<CanvasEvent> {
        let locked = graph.node(id).map(|n| n.flags.locked).unwrap_or(false);
        if locked {
            return vec![CanvasEvent::Refused(
                "node is locked — unlock it from the long-press menu first".to_string(),
            )];
        }
        match graph.op_remove_node(id) {
            Some(op) => {
                self.history.push(op);
                self.selection.clear();
                vec![CanvasEvent::Applied("delete node".to_string())]
            },
            None => vec![CanvasEvent::Refused("no such node".to_string())],
        }
    }

    fn delete_wire(&mut self, graph: &mut Graph, id: WireId) -> Vec<CanvasEvent> {
        match graph.op_remove_wire(id) {
            Some(op) => {
                self.history.push(op);
                self.selection.clear();
                vec![CanvasEvent::Applied("delete wire".to_string())]
            },
            None => vec![CanvasEvent::Refused("no such wire".to_string())],
        }
    }

    fn drag_start(
        &mut self,
        graph: &mut Graph,
        pos: Vec2,
        layout: &CanvasLayout,
    ) -> Vec<CanvasEvent> {
        // An open menu or browser is dismissed by a drag that starts outside it.
        let mut ev = Vec::new();
        if self.browser.is_some() {
            self.browser = None;
            ev.push(CanvasEvent::Browser(false));
        }
        self.menu = None;

        // An inspector slider: the drag edits the parameter continuously (tap-to-set already
        // happened via Activate when the recogniser decided it was a tap, not a drag).
        if let Some((node_id, index, editable, track)) = self.inspector.as_ref().and_then(|il| {
            il.row_at(pos)
                .map(|i| (il.node, il.rows[i].index, il.rows[i].editable, il.rows[i].track))
        }) {
            if !editable {
                ev.push(CanvasEvent::Refused(
                    "that parameter is not editable in v0 — see the inspector row".to_string(),
                ));
                return ev;
            }
            // Set at the grab x immediately (the finger may land off the knob; the value follows
            // the finger from the first pixel — direct manipulation, no jump-on-move).
            if let Some(n) = graph.node(node_id) {
                if let Some(d) = n.spec.params.get(index) {
                    let v = inspector::value_from_x(d, track, pos.x);
                    ev.extend(self.apply_param(graph, node_id, index, v, false));
                }
            }
            self.interaction =
                Interaction::Param { node: node_id, index, cursor_screen: pos, pushed: true };
            return ev;
        }

        let lod = self.camera.lod();
        match layout::hit_test(layout, pos, lod) {
            Hit::Port(pref, dir) => {
                self.interaction = Interaction::Wire(PendingWire {
                    from: pref,
                    from_dir: dir,
                    cursor_screen: pos,
                    hovered: Some(pref),
                });
                ev.push(CanvasEvent::Note("drawing a wire".to_string()));
            },
            Hit::WireEnd(id, side) => {
                let Some(w) = graph.wire(id).copied() else {
                    ev.push(CanvasEvent::Refused("no such wire".to_string()));
                    return ev;
                };
                self.selection.clear();
                self.selection.wires.insert(id);
                self.interaction = Interaction::Repatch {
                    wire: id,
                    side,
                    orig: w,
                    cursor_screen: pos,
                    hovered: None,
                };
                ev.push(CanvasEvent::Note("re-patching a wire end".to_string()));
            },
            Hit::Node(id) => {
                let locked = graph.node(id).map(|n| n.flags.locked).unwrap_or(false);
                if locked {
                    ev.push(CanvasEvent::Refused(
                        "node is locked — unlock it from the long-press menu to move it"
                            .to_string(),
                    ));
                    return ev;
                }
                if !self.selection.nodes.contains(&id) {
                    self.selection.clear();
                    self.selection.nodes.insert(id);
                }
                let orig: Vec<(NodeId, Vec2)> = self
                    .selection
                    .nodes
                    .iter()
                    .filter_map(|nid| graph.node(*nid).map(|n| (n.id, n.pos)))
                    .collect();
                self.interaction = Interaction::Move { orig, acc_screen: Vec2::ZERO };
                ev.push(CanvasEvent::Selection(self.selection.len()));
            },
            Hit::Wire(id) => {
                self.selection.clear();
                self.selection.wires.insert(id);
                // The body selects; the ENDS (grab points) re-patch — both by drag, so the
                // gesture you mean is the gesture you get, and the note says which is which.
                ev.push(CanvasEvent::Selection(1));
                ev.push(CanvasEvent::Note(
                    "wire selected — drag one of its ends to re-patch it".to_string(),
                ));
            },
            Hit::Empty => {
                self.interaction =
                    Interaction::Marquee { start_screen: pos, acc_screen: Vec2::ZERO };
            },
        }
        ev
    }

    fn drag_update(
        &mut self,
        graph: &mut Graph,
        delta: Vec2,
        scale: f32,
        layout: &CanvasLayout,
    ) -> Vec<CanvasEvent> {
        match &mut self.interaction {
            Interaction::Move { orig, acc_screen } => {
                acc_screen.x += delta.x * scale;
                acc_screen.y += delta.y * scale;
                let world =
                    Vec2::new(acc_screen.x / self.camera.zoom, acc_screen.y / self.camera.zoom);
                for (id, base) in orig.iter() {
                    if let Some(n) = graph.node_mut(*id) {
                        n.pos = Vec2::new(base.x + world.x, base.y + world.y);
                    }
                }
            },
            Interaction::Wire(p) => {
                p.cursor_screen =
                    Vec2::new(p.cursor_screen.x + delta.x, p.cursor_screen.y + delta.y);
                // Magnet: capture a port under the cursor.
                let lod = self.camera.lod();
                p.hovered = match layout::hit_test(layout, p.cursor_screen, lod) {
                    Hit::Port(pref, _) => Some(pref),
                    _ => None,
                };
            },
            Interaction::Repatch { cursor_screen, hovered, .. } => {
                *cursor_screen = Vec2::new(cursor_screen.x + delta.x, cursor_screen.y + delta.y);
                // The same magnet as a fresh wire: the drop target is whatever port captures.
                let lod = self.camera.lod();
                *hovered = match layout::hit_test(layout, *cursor_screen, lod) {
                    Hit::Port(pref, _) => Some(pref),
                    _ => None,
                };
            },
            Interaction::Param { node, index, cursor_screen, pushed } => {
                *cursor_screen = Vec2::new(cursor_screen.x + delta.x, cursor_screen.y + delta.y);
                let (node, index, x, pushed) = (*node, *index, cursor_screen.x, *pushed);
                // Geometry comes from this frame's inspector; a stale layout (selection changed
                // mid-drag) simply stops editing rather than guessing.
                let track = self.inspector.as_ref().and_then(|il| {
                    if il.node != node {
                        return None;
                    }
                    il.rows.iter().find(|r| r.index == index).map(|r| r.track)
                });
                let desc = graph.node(node).and_then(|n| n.spec.params.get(index).cloned());
                if let (Some(track), Some(d)) = (track, desc) {
                    let v = inspector::value_from_x(&d, track, x);
                    self.apply_param(graph, node, index, v, pushed);
                }
            },
            Interaction::Marquee { acc_screen, .. } => {
                acc_screen.x += delta.x;
                acc_screen.y += delta.y;
            },
            Interaction::Idle => {},
        }
        Vec::new()
    }

    fn drag_end(
        &mut self,
        graph: &mut Graph,
        pos: Vec2,
        cancelled: bool,
        layout: &CanvasLayout,
        view: Rect,
        ctx: &ConnectContext<'_>,
    ) -> Vec<CanvasEvent> {
        let interaction = std::mem::replace(&mut self.interaction, Interaction::Idle);
        match interaction {
            Interaction::Move { orig, acc_screen } => {
                self.finish_move(graph, orig, acc_screen, cancelled)
            },
            Interaction::Wire(p) => self.finish_wire(graph, p, pos, cancelled, layout, ctx),
            Interaction::Repatch { side, orig, .. } => {
                self.finish_repatch(graph, side, orig, pos, cancelled, layout, ctx)
            },
            Interaction::Param { node, index, .. } => self.finish_param(graph, node, index),
            Interaction::Marquee { start_screen, acc_screen } => {
                self.finish_marquee(graph, start_screen, acc_screen, view)
            },
            Interaction::Idle => Vec::new(),
        }
    }

    /// The slider drag committed: one log line with the final value (mid-drag updates were
    /// silent and coalesced — the history already holds exactly one entry for the gesture).
    fn finish_param(&mut self, graph: &Graph, node: NodeId, index: usize) -> Vec<CanvasEvent> {
        let Some(n) = graph.node(node) else {
            return Vec::new();
        };
        let (Some(d), Some(v)) = (n.spec.params.get(index), n.param_value(index)) else {
            return Vec::new();
        };
        vec![CanvasEvent::Applied(format!("{} = {}", d.name, inspector::value_text(d, v)))]
    }

    /// Drop a re-patched wire end. The verdict runs on the graph with the old wire ALREADY
    /// removed — so the cycle check and the single-input replacement rule judge the world the
    /// re-patch would actually create, not the one it is leaving. A refusal (or a drop on empty
    /// canvas, or a cancel) restores the original wire exactly: the drag was a question, and
    /// "no" leaves everything as it was.
    // The drop-site arguments mirror `finish_wire`'s shape; bundling them into a struct would
    // hide, not reduce, the same eight values (repo precedent: canvas_ui.rs).
    #[allow(clippy::too_many_arguments)]
    fn finish_repatch(
        &mut self,
        graph: &mut Graph,
        side: WireEndSide,
        orig: Wire,
        pos: Vec2,
        cancelled: bool,
        layout: &CanvasLayout,
        ctx: &ConnectContext<'_>,
    ) -> Vec<CanvasEvent> {
        if cancelled {
            return vec![CanvasEvent::Note("re-patch cancelled".to_string())];
        }
        if graph.wire(orig.id).is_none() {
            return vec![CanvasEvent::Refused("that wire no longer exists".to_string())];
        }
        let lod = self.camera.lod();
        let target = match layout::hit_test(layout, pos, lod) {
            Hit::Port(pref, _) => pref,
            _ => {
                return vec![CanvasEvent::Note(
                    "wire end dropped on empty canvas — it snapped back".to_string(),
                )];
            },
        };
        let current_end = if side == WireEndSide::From { orig.from } else { orig.to };
        if target == current_end {
            return vec![CanvasEvent::Note("that end is already there".to_string())];
        }
        let target_dir = graph.port(target).map(|p| p.direction).unwrap_or(Direction::In);
        let (src, dst) = match (side, target_dir) {
            (WireEndSide::From, Direction::Out) => (target, orig.to),
            (WireEndSide::To, Direction::In) => (orig.from, target),
            (WireEndSide::From, _) => {
                return vec![CanvasEvent::Refused(
                    "the source end of a wire lives on an OUTPUT — drop it on another output"
                        .to_string(),
                )];
            },
            (WireEndSide::To, _) => {
                return vec![CanvasEvent::Refused(
                    "the destination end of a wire lives on an INPUT — drop it on another input"
                        .to_string(),
                )];
            },
        };
        // Detach the old wire first (the verdict must see the post-re-patch graph), then ask the
        // matrix. Both mutations ride one Batch into history: one three-finger tap restores the
        // wire exactly where it was.
        let Some(rm) = graph.op_remove_wire(orig.id) else {
            return vec![CanvasEvent::Refused("no such wire".to_string())];
        };
        match connect::resolve(graph, src, dst, ctx) {
            ConnectOutcome::Connected { op, conversion, replaced, adapter } => {
                self.history.push(Op::Batch(vec![rm, op]));
                let mut ev = vec![CanvasEvent::Applied("re-patch".to_string())];
                if replaced {
                    ev.push(CanvasEvent::Note(
                        "replaced the wire on that single input".to_string(),
                    ));
                }
                if conversion {
                    ev.push(CanvasEvent::Note(
                        "summing to mono — warning hairline drawn".to_string(),
                    ));
                }
                if let Some(a) = adapter {
                    ev.push(CanvasEvent::Note(format!("inserted adapter {}", a.module_id())));
                }
                ev
            },
            ConnectOutcome::Refused(r) => {
                graph.apply(&rm.inverse()); // the drag was a question; "no" changes nothing
                vec![CanvasEvent::Refused(r.reason)]
            },
        }
    }

    fn finish_move(
        &mut self,
        graph: &mut Graph,
        orig: Vec<(NodeId, Vec2)>,
        acc_screen: Vec2,
        cancelled: bool,
    ) -> Vec<CanvasEvent> {
        if cancelled {
            for (id, base) in &orig {
                if let Some(n) = graph.node_mut(*id) {
                    n.pos = *base;
                }
            }
            return vec![CanvasEvent::Note("move cancelled".to_string())];
        }
        let grid = LAYOUT_CANVAS_SNAP as f32;
        let world = Vec2::new(acc_screen.x / self.camera.zoom, acc_screen.y / self.camera.zoom);
        let mut ops = Vec::new();
        for (id, base) in &orig {
            let target = snap(Vec2::new(base.x + world.x, base.y + world.y), grid);
            let moved = (target.x - base.x).abs() > f32::EPSILON
                || (target.y - base.y).abs() > f32::EPSILON;
            if let Some(n) = graph.node_mut(*id) {
                n.pos = if moved { target } else { *base };
            }
            if moved {
                ops.push(Op::MoveNode { id: *id, from: *base, to: target });
            }
        }
        if ops.is_empty() {
            return Vec::new();
        }
        let count = ops.len();
        let op =
            if count == 1 { ops.pop().unwrap_or(Op::Batch(Vec::new())) } else { Op::Batch(ops) };
        self.history.push(op);
        vec![CanvasEvent::Applied(format!("move {count} node(s)"))]
    }

    fn finish_wire(
        &mut self,
        graph: &mut Graph,
        p: PendingWire,
        pos: Vec2,
        cancelled: bool,
        layout: &CanvasLayout,
        ctx: &ConnectContext<'_>,
    ) -> Vec<CanvasEvent> {
        if cancelled {
            return vec![CanvasEvent::Note("wire cancelled".to_string())];
        }
        let lod = self.camera.lod();
        let target = match layout::hit_test(layout, pos, lod) {
            Hit::Port(pref, _) => pref,
            _ => {
                return vec![CanvasEvent::Note(
                    "wire dropped on empty canvas — connection cancelled".to_string(),
                )];
            },
        };
        let target_dir = graph.port(target).map(|pt| pt.direction).unwrap_or(Direction::In);
        // Normalise orientation out→in regardless of which end the drag began at.
        let (src, dst) = match (p.from_dir, target_dir) {
            (Direction::Out, Direction::In) => (p.from, target),
            (Direction::In, Direction::Out) => (target, p.from),
            _ => {
                return vec![CanvasEvent::Refused(
                    "a wire runs from an output to an input".to_string(),
                )];
            },
        };
        match connect::resolve(graph, src, dst, ctx) {
            ConnectOutcome::Connected { op, conversion, replaced, adapter } => {
                self.history.push(op);
                let mut ev = vec![CanvasEvent::Applied("connect".to_string())];
                if replaced {
                    ev.push(CanvasEvent::Note(
                        "replaced the wire on that single input".to_string(),
                    ));
                }
                if conversion {
                    ev.push(CanvasEvent::Note(
                        "summing to mono — warning hairline drawn".to_string(),
                    ));
                }
                if let Some(a) = adapter {
                    ev.push(CanvasEvent::Note(format!("inserted adapter {}", a.module_id())));
                }
                ev
            },
            ConnectOutcome::Refused(r) => vec![CanvasEvent::Refused(r.reason)],
        }
    }

    fn finish_marquee(
        &mut self,
        graph: &Graph,
        start_screen: Vec2,
        acc_screen: Vec2,
        view: Rect,
    ) -> Vec<CanvasEvent> {
        let b = Vec2::new(start_screen.x + acc_screen.x, start_screen.y + acc_screen.y);
        let world_a = self.camera.to_world(start_screen, view);
        let world_b = self.camera.to_world(b, view);
        let marquee = Rect::new(
            Vec2::new(world_a.x.min(world_b.x), world_a.y.min(world_b.y)),
            Vec2::new(world_a.x.max(world_b.x), world_a.y.max(world_b.y)),
        );
        self.selection.clear();
        for n in graph.nodes() {
            let nr = Rect::from_min_size(n.pos, layout::node_size(&n.spec));
            if intersects(nr, marquee) {
                self.selection.nodes.insert(n.id);
            }
        }
        vec![CanvasEvent::Selection(self.selection.len())]
    }
}

/// Snap a world point to the grid.
fn snap(v: Vec2, grid: f32) -> Vec2 {
    Vec2::new((v.x / grid).round() * grid, (v.y / grid).round() * grid)
}

/// Whether two rects overlap (marquee selection).
fn intersects(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && a.max.x > b.min.x && a.min.y < b.max.y && a.max.y > b.min.y
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::canvas::connect::ConnectContext;
    use crate::canvas::layout::compute;
    use crate::canvas::model::{NodeSpec, ParamDesc, ParamKind};
    use sparq_module_api::manifest::Port;
    use sparq_module_api::port::Phase;
    use sparq_module_api::port::{ChannelSet, CvRange, CvRate, Multiplicity, PortType};

    fn audio(id: &str, dir: Direction, set: ChannelSet) -> Port {
        Port {
            id: id.into(),
            direction: dir,
            port_type: PortType::Audio,
            required: false,
            channel_set: Some(set),
            channel_set_variable: false,
            cv_rate: None,
            cv_range: None,
            cv_reduce: Default::default(),
            cv_interp: Default::default(),
            event_kinds: Vec::new(),
            multiplicity: Multiplicity::Single,
            latency_contribution: 0,
        }
    }
    fn cv(id: &str, dir: Direction, range: CvRange) -> Port {
        Port {
            id: id.into(),
            direction: dir,
            port_type: PortType::Cv,
            required: false,
            channel_set: None,
            channel_set_variable: false,
            cv_rate: Some(CvRate::Block),
            cv_range: Some(range),
            cv_reduce: Default::default(),
            cv_interp: Default::default(),
            event_kinds: Vec::new(),
            multiplicity: Multiplicity::Single,
            latency_contribution: 0,
        }
    }
    fn sine() -> NodeSpec {
        NodeSpec::new(
            "sparq/syn/sine",
            "Sine",
            vec![audio("out", Direction::Out, ChannelSet::Mono)],
        )
    }
    fn gain() -> NodeSpec {
        NodeSpec::new(
            "sparq/util/gain",
            "Gain",
            vec![
                audio("in", Direction::In, ChannelSet::Stereo),
                audio("out", Direction::Out, ChannelSet::Stereo),
            ],
        )
    }
    fn view() -> Rect {
        Rect::from_min_size(Vec2::ZERO, Vec2::new(1200.0, 800.0))
    }
    fn ctx() -> ConnectContext<'static> {
        ConnectContext::no_adapters(Phase::Zero)
    }

    fn two_nodes() -> (Graph, NodeId, NodeId) {
        let mut g = Graph::new();
        let a = g.op_add_node(sine(), Vec2::new(0.0, 0.0));
        let b = g.op_add_node(gain(), Vec2::new(400.0, 0.0));
        let (aid, bid) = (nid(&a), nid(&b));
        (g, aid, bid)
    }
    fn nid(op: &Op) -> NodeId {
        match op {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        }
    }

    #[test]
    fn tap_a_node_selects_it() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        // centre of node a in screen px
        let na = layout.nodes.iter().find(|n| n.id == aid).unwrap();
        let ev = s.on_intent(
            &mut g,
            GestureIntent::Activate { pos: na.screen.center() },
            &layout,
            v,
            &ctx(),
        );
        assert!(s.selection.nodes.contains(&aid));
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Selection(1))));
    }

    #[test]
    fn drag_from_output_to_input_connects() {
        let (mut g, aid, bid) = two_nodes();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let out = layout
            .nodes
            .iter()
            .find(|n| n.id == aid)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.dir == Direction::Out)
            .unwrap();
        let inp = layout
            .nodes
            .iter()
            .find(|n| n.id == bid)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.dir == Direction::In)
            .unwrap();
        s.on_intent(&mut g, GestureIntent::DragStart { pos: out.screen }, &layout, v, &ctx());
        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate { delta: Vec2::new(10.0, 0.0), scale: 1.0 },
            &layout,
            v,
            &ctx(),
        );
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: inp.screen, cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(g.wire_count(), 1, "a wire was created");
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Applied(_))));
    }

    #[test]
    fn dragging_a_wire_onto_empty_cancels_with_words() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let out = layout
            .nodes
            .iter()
            .find(|n| n.id == aid)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.dir == Direction::Out)
            .unwrap();
        s.on_intent(&mut g, GestureIntent::DragStart { pos: out.screen }, &layout, v, &ctx());
        let empty = Vec2::new(v.max.x - 5.0, v.max.y - 5.0);
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: empty, cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(g.wire_count(), 0);
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Note(m) if m.contains("cancelled"))));
    }

    #[test]
    fn drag_moves_a_node_and_snaps_to_the_grid() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let na = layout.nodes.iter().find(|n| n.id == aid).unwrap();
        let body = na.screen.center();
        s.on_intent(&mut g, GestureIntent::DragStart { pos: body }, &layout, v, &ctx());
        // move 37px right, 11px down at zoom 1 → snaps to 40, 8
        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate { delta: Vec2::new(37.0, 11.0), scale: 1.0 },
            &layout,
            v,
            &ctx(),
        );
        s.on_intent(
            &mut g,
            GestureIntent::DragEnd {
                pos: Vec2::new(body.x + 37.0, body.y + 11.0),
                cancelled: false,
            },
            &layout,
            v,
            &ctx(),
        );
        let p = g.node(aid).unwrap().pos;
        assert_eq!(p, Vec2::new(40.0, 8.0), "snapped to the 8 px grid");
    }

    #[test]
    fn undo_restores_a_move() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let body = layout.nodes.iter().find(|n| n.id == aid).unwrap().screen.center();
        s.on_intent(&mut g, GestureIntent::DragStart { pos: body }, &layout, v, &ctx());
        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate { delta: Vec2::new(80.0, 0.0), scale: 1.0 },
            &layout,
            v,
            &ctx(),
        );
        s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: Vec2::new(body.x + 80.0, body.y), cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(g.node(aid).unwrap().pos.x, 80.0);
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert_eq!(g.node(aid).unwrap().pos.x, 0.0, "undo restored the position");
    }

    #[test]
    fn long_press_opens_a_menu_and_duplicate_copies_the_node() {
        let (mut g, aid, _) = two_nodes();
        let before = g.node_count();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let body = layout.nodes.iter().find(|n| n.id == aid).unwrap().screen.center();
        s.on_intent(&mut g, GestureIntent::Context { pos: body }, &layout, v, &ctx());
        assert!(s.menu.is_some(), "menu opened");
        // tap the DUPLICATE row (row 0)
        let origin = s.menu_origin(v).unwrap();
        let row0 = s.menu.as_ref().unwrap().row_rect(origin, 0);
        // recompute layout is not needed; menu hit uses origin+row_rect
        s.on_intent(&mut g, GestureIntent::Activate { pos: row0.center() }, &layout, v, &ctx());
        assert_eq!(g.node_count(), before + 1, "a copy was added");
        assert!(s.menu.is_none(), "menu closed after the action");
    }

    #[test]
    fn a_cycle_is_refused_through_the_gesture_path() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain(), Vec2::new(0.0, 0.0));
        let b = g.op_add_node(gain(), Vec2::new(400.0, 0.0));
        let (aid, bid) = (nid(&a), nid(&b));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let port = |id: NodeId, dir: Direction| {
            layout
                .nodes
                .iter()
                .find(|n| n.id == id)
                .unwrap()
                .ports
                .iter()
                .find(|p| p.dir == dir)
                .unwrap()
                .screen
        };
        // a.out → b.in
        s.on_intent(
            &mut g,
            GestureIntent::DragStart { pos: port(aid, Direction::Out) },
            &layout,
            v,
            &ctx(),
        );
        s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: port(bid, Direction::In), cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(g.wire_count(), 1);
        // b.out → a.in would close a cycle
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragStart { pos: port(bid, Direction::Out) },
            &layout,
            v,
            &ctx(),
        );
        let _ = ev;
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: port(aid, Direction::In), cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(g.wire_count(), 1, "the cycle-closing wire was refused");
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Refused(m) if m.contains("cycle"))));
    }

    #[test]
    fn marquee_selects_the_nodes_it_covers() {
        let (mut g, aid, bid) = two_nodes();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        // Start the drag on genuinely empty canvas (below-right of both nodes, which sit at the
        // top-left) and sweep up-left across them. Starting at (0,0) would land ON node a and
        // become a move, not a marquee — the hit-test prefers nodes over empty space.
        let start = Vec2::new(700.0, 300.0);
        let end = Vec2::new(0.0, 10.0);
        s.on_intent(&mut g, GestureIntent::DragStart { pos: start }, &layout, v, &ctx());
        assert!(matches!(s.interaction, Interaction::Marquee { .. }), "empty start → marquee");
        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate {
                delta: Vec2::new(end.x - start.x, end.y - start.y),
                scale: 1.0,
            },
            &layout,
            v,
            &ctx(),
        );
        s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: end, cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert!(s.selection.nodes.contains(&aid) && s.selection.nodes.contains(&bid));
    }

    #[test]
    fn resolve_master_prefers_a_wired_terminus_then_any_wired_audio_out() {
        // Shape 1: the demo patch — sine → gain → rms, plus an UNWIRED spare gain. The master
        // must be the gain that feeds rms (audio out, wired, and rms is not a candidate: no
        // audio out). The spare is excluded: rendering it would be silence and confusion.
        let mut g = Graph::new();
        let s_op = g.op_add_node(sine(), Vec2::new(0.0, 0.0));
        let g_op = g.op_add_node(gain(), Vec2::new(400.0, 0.0));
        let r_op = g.op_add_node(rms_like(), Vec2::new(800.0, 0.0));
        let _spare = g.op_add_node(gain(), Vec2::new(400.0, 200.0));
        let (sid, gid, rid) = (nid(&s_op), nid(&g_op), nid(&r_op));
        g.op_add_wire(PortRef::new(sid, 0), PortRef::new(gid, 0));
        g.op_add_wire(PortRef::new(gid, 1), PortRef::new(rid, 0));
        let s = CanvasState::new();
        assert_eq!(s.resolve_master(&g), Some(gid), "the gain feeds the listener");

        // Shape 2: a chain a → b → c of audio nodes: the terminus wins even though b also has
        // an outgoing wire.
        let mut g2 = Graph::new();
        let a = g2.op_add_node(gain(), Vec2::new(0.0, 0.0));
        let b = g2.op_add_node(gain(), Vec2::new(400.0, 0.0));
        let c = g2.op_add_node(gain(), Vec2::new(800.0, 0.0));
        let (aid, bid, cid) = (nid(&a), nid(&b), nid(&c));
        g2.op_add_wire(PortRef::new(aid, 1), PortRef::new(bid, 0));
        g2.op_add_wire(PortRef::new(bid, 1), PortRef::new(cid, 0));
        assert_eq!(s.resolve_master(&g2), Some(cid), "the chain's terminus is the master");

        // Shape 3: no audio outputs at all → None, and the bridge must refuse in words.
        let mut g3 = Graph::new();
        let r = g3.op_add_node(rms_like(), Vec2::ZERO);
        let _ = r;
        assert_eq!(s.resolve_master(&g3), None);
    }

    #[test]
    fn an_out_main_node_supersedes_the_default_master_rule() {
        // WO-014 inc 5's handover: a patch with a real output module resolves to THAT node, by
        // name, not to the highest-id terminus the default rule would guess. Here `later` is a
        // wired gain terminus with the higher id — exactly what the old rule picked — and the
        // wired `out/main` must win instead, so the MASTER badge never lies.
        let mut g = Graph::new();
        let s_op = g.op_add_node(sine(), Vec2::new(0.0, 0.0));
        let g_op = g.op_add_node(gain(), Vec2::new(400.0, 0.0));
        let o_op = g.op_add_node(out_main(), Vec2::new(800.0, 0.0));
        let l_op = g.op_add_node(gain(), Vec2::new(800.0, 200.0));
        let (sid, gid, oid, lid) = (nid(&s_op), nid(&g_op), nid(&o_op), nid(&l_op));
        g.op_add_wire(PortRef::new(sid, 0), PortRef::new(gid, 0));
        g.op_add_wire(PortRef::new(gid, 1), PortRef::new(oid, 0));
        g.op_add_wire(PortRef::new(gid, 1), PortRef::new(lid, 0));
        assert!(lid > oid, "the competing terminus has the higher id the default rule preferred");
        let s = CanvasState::new();
        assert_eq!(s.resolve_master(&g), Some(oid), "out/main IS the master, by name");
    }

    #[test]
    fn an_explicit_master_still_beats_an_out_main_node() {
        // The handover is the DEFAULT, not an override of the user: SET MASTER on another node
        // still wins, because an explicit choice is a statement the rule must not contradict.
        let mut g = Graph::new();
        let s_op = g.op_add_node(sine(), Vec2::new(0.0, 0.0));
        let o_op = g.op_add_node(out_main(), Vec2::new(400.0, 0.0));
        let (sid, oid) = (nid(&s_op), nid(&o_op));
        g.op_add_wire(PortRef::new(sid, 0), PortRef::new(oid, 0));
        let mut s = CanvasState::new();
        s.master = Some(sid);
        assert_eq!(s.resolve_master(&g), Some(sid), "explicit beats the out/main default");
    }

    #[test]
    fn an_unwired_out_main_does_not_become_master() {
        // A spare out/main with no wires would render silence, so it does not count; the rule
        // falls through to the wired terminus. Same exclusion the default rule applies.
        let mut g = Graph::new();
        let s_op = g.op_add_node(sine(), Vec2::ZERO);
        let g_op = g.op_add_node(gain(), Vec2::new(400.0, 0.0));
        let _o = g.op_add_node(out_main(), Vec2::new(800.0, 0.0));
        let (sid, gid) = (nid(&s_op), nid(&g_op));
        g.op_add_wire(PortRef::new(sid, 0), PortRef::new(gid, 0));
        let s = CanvasState::new();
        assert_eq!(s.resolve_master(&g), Some(gid), "the wired gain, not the unwired out/main");
    }

    #[test]
    fn an_explicit_master_wins_and_a_deleted_master_falls_back() {
        let mut g = Graph::new();
        let s_op = g.op_add_node(sine(), Vec2::new(0.0, 0.0));
        let g_op = g.op_add_node(gain(), Vec2::new(400.0, 0.0));
        let (sid, gid) = (nid(&s_op), nid(&g_op));
        g.op_add_wire(PortRef::new(sid, 0), PortRef::new(gid, 0));
        let mut s = CanvasState::new();
        s.master = Some(sid);
        assert_eq!(s.resolve_master(&g), Some(sid), "explicit beats the rule");
        g.op_remove_node(sid);
        assert_eq!(
            s.resolve_master(&g),
            None,
            "a deleted explicit master is not resurrected; gain alone has no wire now"
        );
    }

    #[test]
    fn the_empty_menu_offers_render_and_the_node_menu_offers_set_master() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        // Empty-canvas long press → RENDER WAV row exists; tapping it emits the event.
        let empty = Vec2::new(v.max.x - 10.0, v.max.y - 10.0);
        s.on_intent(&mut g, GestureIntent::Context { pos: empty }, &layout, v, &ctx());
        let menu = s.menu.clone().expect("menu open");
        let row = menu
            .rows
            .iter()
            .position(|r| r.action == MenuAction::RenderWav)
            .expect("RENDER WAV row");
        let origin = s.menu_origin(v).unwrap();
        let tap = menu.row_rect(origin, row).center();
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tap }, &layout, v, &ctx());
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::RenderWav)), "render event: {ev:?}");
        // Node long press → SET MASTER sets the master.
        let body = layout.nodes.iter().find(|n| n.id == aid).unwrap().screen.center();
        s.on_intent(&mut g, GestureIntent::Context { pos: body }, &layout, v, &ctx());
        let menu = s.menu.clone().expect("menu open");
        let row = menu
            .rows
            .iter()
            .position(|r| r.action == MenuAction::SetMaster)
            .expect("SET MASTER row");
        let origin = s.menu_origin(v).unwrap();
        let tap = menu.row_rect(origin, row).center();
        s.on_intent(&mut g, GestureIntent::Activate { pos: tap }, &layout, v, &ctx());
        assert_eq!(s.master, Some(aid));
    }

    fn rms_like() -> NodeSpec {
        NodeSpec::new(
            "sparq/ana/rms",
            "RMS",
            vec![
                audio("in", Direction::In, ChannelSet::Stereo),
                cv("level", Direction::Out, CvRange::Unipolar),
            ],
        )
    }

    fn out_main() -> NodeSpec {
        NodeSpec::new(
            crate::canvas::OUT_MAIN_ID,
            "Main Out",
            vec![
                audio("in", Direction::In, ChannelSet::Variable),
                audio("out", Direction::Out, ChannelSet::Stereo),
            ],
        )
    }

    #[test]
    fn cv_into_audio_is_refused_in_words_at_phase_zero() {
        let mut g = Graph::new();
        let src = g.op_add_node(
            NodeSpec::new("sparq/x", "CV", vec![cv("o", Direction::Out, CvRange::Unipolar)]),
            Vec2::ZERO,
        );
        let dst = g.op_add_node(
            NodeSpec::new("sparq/y", "Sink", vec![audio("i", Direction::In, ChannelSet::Mono)]),
            Vec2::new(400.0, 0.0),
        );
        let (sid, did) = (nid(&src), nid(&dst));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let outp = layout.nodes.iter().find(|n| n.id == sid).unwrap().ports[0].screen;
        let inp = layout.nodes.iter().find(|n| n.id == did).unwrap().ports[0].screen;
        s.on_intent(&mut g, GestureIntent::DragStart { pos: outp }, &layout, v, &ctx());
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: inp, cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(g.wire_count(), 0);
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Refused(_))));
    }

    // --------------------------------------- increment 3: browser, inspector, wire re-patch

    fn freq_param() -> ParamDesc {
        ParamDesc {
            id: "freq".into(),
            name: "Frequency".into(),
            kind: ParamKind::Float,
            unit: Some("Hz".into()),
            min: 0.0,
            max: 24_000.0,
            default: 440.0,
        }
    }

    fn sine_with_params() -> NodeSpec {
        sine().with_params(vec![freq_param()])
    }

    fn catalogue() -> Vec<BrowserItem> {
        vec![
            BrowserItem {
                spec: sine_with_params(),
                summary: "Exact-frequency sine oscillator".into(),
                category: "synth/oscillator/sine".into(),
            },
            BrowserItem::new(gain()),
            BrowserItem::new(rms_like()),
        ]
    }

    #[test]
    fn the_empty_menu_opens_the_browser_and_a_row_tap_spawns_the_module() {
        let (mut g, _, _) = two_nodes();
        let mut s = CanvasState::new();
        s.set_catalog(catalogue());
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let empty = Vec2::new(v.max.x - 10.0, v.max.y - 10.0);
        // long-press the empty canvas → the menu offers ADD MODULE (first row)
        s.on_intent(&mut g, GestureIntent::Context { pos: empty }, &layout, v, &ctx());
        let menu = s.menu.clone().expect("menu open");
        let row = menu
            .rows
            .iter()
            .position(|r| r.action == MenuAction::OpenBrowser)
            .expect("ADD MODULE row");
        assert!(menu.rows[row].enabled, "a non-empty catalogue enables the row");
        let origin = s.menu_origin(v).unwrap();
        let tap = menu.row_rect(origin, row).center();
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tap }, &layout, v, &ctx());
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Browser(true))), "{ev:?}");
        assert!(s.browser.is_some(), "the browser is open");

        // type "si" → Sine (name match) first; Gain's id `sparq/util/gain` also contains s…i as
        // a subsequence, honestly — a name match outranks an id match, so rank 0 is Sine.
        let ev = s.browser_set_query("si");
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::BrowserQuery(2))), "{ev:?}");
        let (max_w, max_h) = crate::canvas::browser::caps(v);
        let b = s.browser.as_ref().unwrap();
        assert_eq!(b.selected_item().unwrap().spec.module_id, "sparq/syn/sine");
        let origin = b.sheet_origin(v, max_w, max_h);
        let row0 = b.row_rect(origin, 0, v, max_w).center();

        // tap the row → the module spawns at the press, selected, browser closed
        let before = g.node_count();
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: row0 }, &layout, v, &ctx());
        assert_eq!(g.node_count(), before + 1, "a node was spawned");
        let spawned = g.nodes().iter().last().unwrap();
        assert_eq!(spawned.spec.module_id, "sparq/syn/sine");
        assert_eq!(spawned.spec.params.len(), 1, "the spec carried its params");
        assert!(s.browser.is_none(), "one tap = one module, back to the canvas");
        assert!(s.selection.nodes.contains(&spawned.id), "the new node is selected");
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Applied(t) if t.contains("Sine"))),
            "{ev:?}"
        );
        // it spawned where the menu was anchored, on the grid
        let world = s.camera.to_world(empty, v);
        assert!((spawned.pos.x - world.x).abs() < 64.0 && (spawned.pos.y - world.y).abs() < 64.0);
        assert_eq!(spawned.pos.x % 8.0, 0.0, "snapped to the grid");
    }

    #[test]
    fn a_second_spawn_cascades_off_the_first_instead_of_stacking() {
        let (mut g, _, _) = two_nodes();
        let mut s = CanvasState::new();
        s.set_catalog(catalogue());
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let empty = Vec2::new(v.max.x - 10.0, v.max.y - 10.0);
        for _ in 0..2 {
            s.on_intent(&mut g, GestureIntent::Context { pos: empty }, &layout, v, &ctx());
            let menu = s.menu.clone().unwrap();
            let row = menu.rows.iter().position(|r| r.action == MenuAction::OpenBrowser).unwrap();
            let origin = s.menu_origin(v).unwrap();
            s.on_intent(
                &mut g,
                GestureIntent::Activate { pos: menu.row_rect(origin, row).center() },
                &layout,
                v,
                &ctx(),
            );
            let (max_w, max_h) = crate::canvas::browser::caps(v);
            let b = s.browser.as_ref().unwrap();
            let origin = b.sheet_origin(v, max_w, max_h);
            let row0 = b.row_rect(origin, 0, v, max_w).center();
            s.on_intent(&mut g, GestureIntent::Activate { pos: row0 }, &layout, v, &ctx());
        }
        let spawned: Vec<_> = g.nodes().iter().skip(2).collect();
        assert_eq!(spawned.len(), 2);
        assert_ne!(spawned[0].pos, spawned[1].pos, "the cascade kept them apart");
    }

    #[test]
    fn no_catalogue_no_add_module() {
        let (mut g, _, _) = two_nodes();
        let mut s = CanvasState::new(); // no catalogue supplied
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let empty = Vec2::new(v.max.x - 10.0, v.max.y - 10.0);
        s.on_intent(&mut g, GestureIntent::Context { pos: empty }, &layout, v, &ctx());
        let menu = s.menu.clone().unwrap();
        let row = menu.rows.iter().position(|r| r.action == MenuAction::OpenBrowser).unwrap();
        assert!(!menu.rows[row].enabled, "#58: never offer what is not installed");
        let origin = s.menu_origin(v).unwrap();
        let ev = s.on_intent(
            &mut g,
            GestureIntent::Activate { pos: menu.row_rect(origin, row).center() },
            &layout,
            v,
            &ctx(),
        );
        assert!(s.browser.is_none());
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Refused(_))), "{ev:?}");
    }

    #[test]
    fn a_no_match_query_keeps_the_browser_open_and_says_why() {
        let (mut g, _, _) = two_nodes();
        let mut s = CanvasState::new();
        s.set_catalog(catalogue());
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let empty = Vec2::new(v.max.x - 10.0, v.max.y - 10.0);
        s.on_intent(&mut g, GestureIntent::Context { pos: empty }, &layout, v, &ctx());
        let menu = s.menu.clone().unwrap();
        let row = menu.rows.iter().position(|r| r.action == MenuAction::OpenBrowser).unwrap();
        let origin = s.menu_origin(v).unwrap();
        s.on_intent(
            &mut g,
            GestureIntent::Activate { pos: menu.row_rect(origin, row).center() },
            &layout,
            v,
            &ctx(),
        );
        s.browser_set_query("zzz");
        let (max_w, max_h) = crate::canvas::browser::caps(v);
        let b = s.browser.as_ref().unwrap();
        let origin = b.sheet_origin(v, max_w, max_h);
        // the NO-MATCH page still has a row slot; tapping it must refuse, not spawn
        let row0 = b.row_rect(origin, 0, v, max_w).center();
        let before = g.node_count();
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: row0 }, &layout, v, &ctx());
        assert_eq!(g.node_count(), before, "nothing spawned");
        assert!(s.browser.is_some(), "the browser stays open — the empty list is the answer");
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Refused(_))), "{ev:?}");
    }

    #[test]
    fn dragging_a_wires_end_repatches_it_and_one_undo_restores_the_original() {
        let mut g = Graph::new();
        let s_id = nid(&g.op_add_node(sine(), Vec2::new(0.0, 0.0)));
        let a_id = nid(&g.op_add_node(gain(), Vec2::new(400.0, 0.0)));
        let b_id = nid(&g.op_add_node(gain(), Vec2::new(400.0, 300.0)));
        g.op_add_wire(PortRef::new(s_id, 0), PortRef::new(a_id, 0));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let grab = layout.wires[0].grab_to;
        let b_in = layout
            .nodes
            .iter()
            .find(|n| n.id == b_id)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.dir == Direction::In)
            .unwrap()
            .screen;
        s.on_intent(&mut g, GestureIntent::DragStart { pos: grab }, &layout, v, &ctx());
        assert!(
            matches!(s.interaction, Interaction::Repatch { side: WireEndSide::To, .. }),
            "{:?}",
            s.interaction
        );
        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate { delta: b_in.sub(grab), scale: 1.0 },
            &layout,
            v,
            &ctx(),
        );
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: b_in, cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Applied(t) if t == "re-patch")),
            "{ev:?}"
        );
        assert_eq!(g.wire_count(), 1);
        let new_id = g.wires()[0].id;
        assert_ne!(new_id, 0, "the re-patch made a NEW wire (the old one was removed)");
        assert_eq!(g.wires()[0].from, PortRef::new(s_id, 0), "the source end stayed put");
        assert_eq!(
            g.wires()[0].to,
            PortRef::new(b_id, 0),
            "the destination moved to the spare gain"
        );
        // ONE undo restores the original wire — id 0, original ends (the batch is the unit)
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert_eq!(g.wire_count(), 1);
        assert_eq!(g.wires()[0].id, 0, "the ORIGINAL wire is back, same id, not a copy");
        assert_eq!(g.wires()[0].to, PortRef::new(a_id, 0));
    }

    #[test]
    fn a_repatch_dropped_on_empty_canvas_snaps_back() {
        let mut g = Graph::new();
        let s_id = nid(&g.op_add_node(sine(), Vec2::new(0.0, 0.0)));
        let a_id = nid(&g.op_add_node(gain(), Vec2::new(400.0, 0.0)));
        g.op_add_wire(PortRef::new(s_id, 0), PortRef::new(a_id, 0));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let grab = layout.wires[0].grab_to;
        s.on_intent(&mut g, GestureIntent::DragStart { pos: grab }, &layout, v, &ctx());
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: Vec2::new(600.0, 700.0), cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Note(t) if t.contains("snapped back"))),
            "{ev:?}"
        );
        assert_eq!(g.wires()[0].to, PortRef::new(a_id, 0), "untouched");
        assert!(!s.history.can_undo(), "a snap-back is not an operation");
    }

    #[test]
    fn a_repatch_that_would_close_a_cycle_is_refused_and_restores_the_wire() {
        // s → a → b; re-patch wire s→a's FROM end onto b's OUTPUT: a→b→a cycle → refused,
        // and the original wire must survive byte-for-byte (id included).
        let mut g = Graph::new();
        let s_id = nid(&g.op_add_node(sine(), Vec2::new(0.0, 0.0)));
        let a_id = nid(&g.op_add_node(gain(), Vec2::new(400.0, 0.0)));
        let b_id = nid(&g.op_add_node(gain(), Vec2::new(400.0, 300.0)));
        g.op_add_wire(PortRef::new(s_id, 0), PortRef::new(a_id, 0));
        g.op_add_wire(PortRef::new(a_id, 1), PortRef::new(b_id, 0));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let grab = layout.wires[0].grab_from;
        let b_out = layout
            .nodes
            .iter()
            .find(|n| n.id == b_id)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.dir == Direction::Out)
            .unwrap()
            .screen;
        let id_before = g.wires()[0].id;
        s.on_intent(&mut g, GestureIntent::DragStart { pos: grab }, &layout, v, &ctx());
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: b_out, cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("cycle"))),
            "{ev:?}"
        );
        assert_eq!(g.wire_count(), 2, "nothing was lost");
        // (the restore re-appends, so DRAW ORDER can move — identity and ends must not)
        let w = g.wire(id_before).expect("the original wire, same id");
        assert_eq!(w.from, PortRef::new(s_id, 0));
        assert_eq!(w.to, PortRef::new(a_id, 0));
    }

    #[test]
    fn dropping_the_wrong_end_on_the_wrong_direction_says_so() {
        let mut g = Graph::new();
        let s_id = nid(&g.op_add_node(sine(), Vec2::new(0.0, 0.0)));
        let a_id = nid(&g.op_add_node(gain(), Vec2::new(400.0, 0.0)));
        let b_id = nid(&g.op_add_node(gain(), Vec2::new(400.0, 300.0)));
        g.op_add_wire(PortRef::new(s_id, 0), PortRef::new(a_id, 0));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let grab = layout.wires[0].grab_to; // the INPUT end…
        let b_out = layout
            .nodes
            .iter()
            .find(|n| n.id == b_id)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.dir == Direction::Out)
            .unwrap()
            .screen; // …dropped on an OUTPUT
        s.on_intent(&mut g, GestureIntent::DragStart { pos: grab }, &layout, v, &ctx());
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: b_out, cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("INPUT"))),
            "{ev:?}"
        );
        assert_eq!(g.wires()[0].to, PortRef::new(a_id, 0), "restored");
    }

    #[test]
    fn inspector_tap_sets_a_param_and_one_drag_is_one_undo_step() {
        let mut g = Graph::new();
        let id = nid(&g.op_add_node(sine_with_params(), Vec2::ZERO));
        let mut s = CanvasState::new();
        let panel = Rect::from_min_size(Vec2::new(800.0, 100.0), Vec2::new(400.0, 600.0));
        let il = inspector::compute(g.node(id).unwrap(), panel);
        s.set_inspector(Some(il.clone()));
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let row = il.rows[0];

        // tap at 3/4 of the track → freq = 18 000 Hz, node selected, one history entry
        let tap = Vec2::new(row.track.min.x + row.track.width() * 0.75, row.track.center().y);
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tap }, &layout, v, &ctx());
        let got = g.node(id).unwrap().param_value(0).unwrap();
        assert!((got - 18_000.0).abs() < 1.0, "{got}");
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Applied(t) if t.contains("Frequency"))),
            "{ev:?}"
        );
        assert!(s.selection.nodes.contains(&id), "editing selects the node");

        // drag from 1/4 to 1/2 of the track: many updates, ONE history entry
        let undo_before = s.history.undo_len();
        let start = Vec2::new(row.track.min.x + row.track.width() * 0.25, row.track.center().y);
        s.on_intent(&mut g, GestureIntent::DragStart { pos: start }, &layout, v, &ctx());
        assert!(matches!(s.interaction, Interaction::Param { .. }), "{:?}", s.interaction);
        for _ in 0..5 {
            s.on_intent(
                &mut g,
                GestureIntent::DragUpdate {
                    delta: Vec2::new(row.track.width() * 0.05, 0.0),
                    scale: 1.0,
                },
                &layout,
                v,
                &ctx(),
            );
        }
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd {
                pos: Vec2::new(start.x + row.track.width() * 0.25, start.y),
                cancelled: false,
            },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(s.history.undo_len(), undo_before + 1, "one drag = one undo step");
        let dragged = g.node(id).unwrap().param_value(0).unwrap();
        assert!((dragged - 12_000.0).abs() < 600.0, "{dragged} ≈ half of 24 kHz");
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Applied(t) if t.contains("12000"))),
            "the final value is logged once, at the end: {ev:?}"
        );

        // undo → the tap value; undo → the default (param state rides the graph history)
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert!((g.node(id).unwrap().param_value(0).unwrap() - 18_000.0).abs() < 1.0);
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert!((g.node(id).unwrap().param_value(0).unwrap() - 440.0).abs() < 1.0, "default");
    }

    #[test]
    fn the_inspector_refuses_a_non_editable_param_in_words() {
        let mut g = Graph::new();
        let spec = sine().with_params(vec![ParamDesc {
            id: "mode".into(),
            name: "Mode".into(),
            kind: ParamKind::Enum,
            unit: None,
            min: 0.0,
            max: 0.0,
            default: 0.0,
        }]);
        let id = nid(&g.op_add_node(spec, Vec2::ZERO));
        let mut s = CanvasState::new();
        let panel = Rect::from_min_size(Vec2::new(800.0, 100.0), Vec2::new(400.0, 600.0));
        let il = inspector::compute(g.node(id).unwrap(), panel);
        s.set_inspector(Some(il.clone()));
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let tap = il.rows[0].track.center();
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tap }, &layout, v, &ctx());
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("not editable"))),
            "{ev:?}"
        );
        assert!(!s.history.can_undo(), "a refusal is not an operation");
    }
}
