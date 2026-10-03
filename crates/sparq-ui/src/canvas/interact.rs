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
use crate::canvas::layout::{self, CanvasLayout, Hit, SignalClass, WireEndSide};
use crate::canvas::model::{Graph, NodeId, Op, PortRef, UndoStack, Wire, WireId, WireTrim};
use crate::geom::{Rect, Vec2};
use crate::gesture::GestureIntent;
use crate::tokens::{
    LAYOUT_CANVAS_SNAP, LAYOUT_SPACE_2, LAYOUT_SPACE_3, LAYOUT_SPACE_PADDING_SECTION,
    LAYOUT_TOUCH_PORT_CAPTURE_RADIUS, LAYOUT_TOUCH_ROW_HEIGHT_LIST,
};
use sparq_module_api::port::{Direction, Phase};

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
    /// The parameter SINK the cursor is capturing (operator ruling 2026-10-01 r3): while a
    /// control (cv) drag is in flight, the hovered module's float settings wear small dots,
    /// and this is the magnet for them.
    pub hovered_param: Option<(NodeId, usize)>,
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
    /// Dragging a wire's CABLE NODE (operator round 4, D15): vertical is amp (up louder,
    /// 100 px per unit), horizontal is offset (right positive, 100 px per unit) — the offset
    /// pinned 0 on audio wires, because DC never enters the audio path. `orig` is the trim
    /// the gesture found (the cancel's restore data); the drag coalesces into ONE history
    /// entry keeping that original `from`, the `Param` drag's discipline.
    Trim {
        /// The wire whose trim is being edited.
        wire: WireId,
        /// The trim as the gesture found it.
        orig: Option<WireTrim>,
        /// Accumulated screen delta.
        acc_screen: Vec2,
        /// Whether the wire carries audio (its offset stays 0 — D15).
        audio: bool,
    },
    /// Dragging the inspector's response-plot marker (WO-012 increment 5, D3). A READ-ONLY
    /// probe: it moves no param, pushes no history, notes nothing on the live-sync ledger —
    /// the marker is display state, exactly like the camera and the scroll offset.
    Marker {
        /// The node whose curve is being probed.
        node: NodeId,
        /// Where the finger is now, screen px.
        cursor_screen: Vec2,
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
    /// Open the rename text-entry sheet for this node (increment 5). The sheet commits an
    /// [`Op::Rename`] — the value and its inverse have been in the model since increment 1;
    /// this is the surface that was parked until a text entry existed.
    Rename,
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
    /// The rename text-entry sheet opened (`true`) or closed (`false`) — increment 5.
    Rename(bool),
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
            Self::Rename(true) => {
                "canvas: rename entry open — type, ENTER commits, ESC cancels".to_string()
            },
            Self::Rename(false) => "canvas: rename entry closed".to_string(),
        }
    }
}

/// What the patch did since the last [`CanvasState::take_patch_changes`], in the only two
/// vocabularies a LIVE engine cares about (WO-012 increment 2, the one op→sync door): a
/// STRUCTURAL change (nodes, wires, flags, the master) means a live session must rebuild and
/// re-stage the executor; a PARAM edit means it can send a fresh snapshot over the command ring
/// instead — no re-stage, no reset of module state, no click mid-drag. Ops the engine cannot
/// hear (`MoveNode`, `Rename`) mark NOTHING: a sync that rebuilds for a pixel move is a lie
/// about what changed. Toolkit-independent and bounded: `param_nodes` dedupes, so the ledger
/// cannot grow past the graph even if nobody takes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PatchChanges {
    /// Any node/wire/flag/master change happened — rebuild + re-stage is owed.
    pub structural: bool,
    /// Nodes whose params were edited — each owes a `set_params` snapshot, not a re-stage.
    pub param_nodes: Vec<NodeId>,
}

impl PatchChanges {
    /// Nothing happened: the value `take_patch_changes` leaves behind.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.structural && self.param_nodes.is_empty()
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
    /// The open rename text-entry sheet, if any (increment 5). The DEEPEST modal: it is opened
    /// FROM the menu, so while it is open it captures taps (an outside tap cancels) and the
    /// shell's key feed (type → buffer, Enter → commit, Escape → cancel). Like the browser, its
    /// keyboard path is an input-event feed, not a toolkit widget — the wrap-egui rule holds.
    pub rename: Option<crate::canvas::entry::RenameState>,
    /// The inspector's requested scroll offset, px (increment 5). TRANSIENT view state like
    /// `levels` — not undoable, because where a panel is scrolled is not an edit to the patch.
    /// The layout clamps it every frame; it resets when the inspected node changes.
    insp_scroll: f32,
    /// Which node `insp_scroll` belongs to — a selection change starts the new panel at the top.
    insp_node: Option<NodeId>,
    /// The response plot's probe marker (WO-012 increment 5, D3): `(node, frequency in Hz)`.
    /// DISPLAY state beside the camera and the scroll offset — NOT undoable (it edits no
    /// param), reset when the inspected node changes or the selection closes. Private: the
    /// drag/tap path and [`Self::set_response_marker`] are the only doors, so the clamp and the
    /// node key cannot be bypassed.
    response_marker: Option<(NodeId, f64)>,
    /// The axes the shell computed this frame's response curve with, per inspected node — the
    /// marker drag's x → frequency mapping (the layout's own log scale, never a second copy).
    /// `None` while no curve module is inspected.
    resp_axes: Option<(NodeId, crate::canvas::response::Axes)>,
    /// The module catalogue the browser ranks: what the registry actually has, supplied by the
    /// shell at startup — the canvas never invents modules (defect #58, structurally).
    catalog: Vec<BrowserItem>,
    /// The inspector panel geometry for the current selection, recomputed by the shell each
    /// frame (`None` when nothing is selected or the panel is collapsed). The canvas reads it to
    /// route slider drags; the painter reads it to draw.
    inspector: Option<InspectorLayout>,
    /// Wires draw as straight runs instead of béziers (increment 6 toolbar select). Display
    /// state: not undoable, not on the live-sync ledger, the painter and the hit-test both read
    /// the restyled polyline so what you see is what you can grab.
    pub wire_straight: bool,
    /// Per-node signal levels for the live wire-level animation (WO-013 increment 4). TRANSIENT
    /// and NOT part of the undoable model: a level is a fact about the last render, not an edit.
    /// The shell refreshes it from the executor's meters (the bridge reads them); the painter maps
    /// it onto wires through [`crate::canvas::levels::wire_level`]. Empty until the first render,
    /// so wires draw at rest exactly as they did before this increment — no faked levels.
    pub levels: crate::canvas::levels::NodeLevels,
    /// The live-sync ledger: what the engine can hear since the last take. TRANSIENT like
    /// `levels` — not undoable, because it is a record of edits already in the history, kept
    /// only so the sync door reads each change exactly once. Private: every mutation site goes
    /// through `commit`/`note_patch`, and the shell reads it through `take_patch_changes`.
    patch_changes: PatchChanges,
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
            rename: None,
            catalog: Vec::new(),
            inspector: None,
            insp_scroll: 0.0,
            insp_node: None,
            response_marker: None,
            resp_axes: None,
            wire_straight: false,
            levels: crate::canvas::levels::NodeLevels::new(),
            patch_changes: PatchChanges::default(),
        }
    }

    /// Take everything the engine can hear about since the last call, leaving an empty ledger
    /// behind. The live session calls this once per frame while it plays: `structural` owes a
    /// rebuild + re-stage, each node in `param_nodes` owes a `set_params` snapshot over the
    /// command ring. Bounded whether or not anybody takes — `note_patch` dedupes param nodes,
    /// so an idle ledger cannot grow past the graph.
    pub fn take_patch_changes(&mut self) -> PatchChanges {
        std::mem::take(&mut self.patch_changes)
    }

    /// Record what one op means to a LIVE engine. The classification is exhaustive on purpose:
    /// a new `Op` variant fails to compile here until somebody decides what it sounds like,
    /// rather than silently never reaching the audio thread.
    fn note_patch(&mut self, op: &Op) {
        match op {
            // The engine cannot hear a pixel move or a label.
            Op::MoveNode { .. } | Op::Rename { .. } => {},
            Op::SetParam { node, .. } => {
                if !self.patch_changes.param_nodes.contains(node) {
                    self.patch_changes.param_nodes.push(*node);
                }
            },
            Op::Batch(ops) => {
                for o in ops {
                    self.note_patch(o);
                }
            },
            Op::AddNode(_)
            | Op::RemoveNode { .. }
            | Op::AddWire(_)
            | Op::RemoveWire(_)
            | Op::SetFlags { .. }
            // A trim re-stages (the door's decided sentence, operator round 4 D15): the
            // bridge's kernel shape moves — a gain node appears or disappears for an audio
            // trim, the executor's cv trim or the param-mod formula changes for the others.
            // Silent to the listener since D1's adoption; a command-ring fast path for audio
            // trims is a declared LATER optimisation, not today's lie.
            | Op::SetTrim { .. } => self.patch_changes.structural = true,
        }
    }

    /// Commit one op to the undo history AND to the live-sync ledger — the single door every
    /// edit goes through, which is what makes "ops apply to the canvas first, then sync" one
    /// sentence instead of a per-call-site obligation.
    fn commit(&mut self, op: Op) {
        self.note_patch(&op);
        self.history.push(op);
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

    // ------------------------------------------------------------ rename entry (increment 5)

    /// The open rename sheet, for the painter.
    #[must_use]
    pub fn rename(&self) -> Option<&crate::canvas::entry::RenameState> {
        self.rename.as_ref()
    }

    /// Open the rename sheet for `node`, anchored where the menu was. Refuses a missing node in
    /// words — the same rule every menu action obeys.
    pub fn rename_open(&mut self, graph: &Graph, id: NodeId, anchor: Vec2) -> Vec<CanvasEvent> {
        let Some(node) = graph.node(id) else {
            return vec![CanvasEvent::Refused("no such node".to_string())];
        };
        self.rename = Some(crate::canvas::entry::RenameState::open(
            id,
            node.spec.module_id.as_str(),
            node.title(),
            anchor,
        ));
        vec![CanvasEvent::Rename(true)]
    }

    /// Typed text into the open sheet (the shell pipes printables here). Silent: the sheet shows
    /// the buffer with its caret, so the feedback is the thing being edited, not a log line.
    pub fn rename_insert(&mut self, text: &str) {
        if let Some(r) = &mut self.rename {
            r.entry.insert(text);
        }
    }

    /// One backspace into the open sheet.
    pub fn rename_backspace(&mut self) {
        if let Some(r) = &mut self.rename {
            r.entry.backspace();
        }
    }

    /// Open the browser sheet at an arbitrary anchor — the documented driver door (the
    /// `browser_set_query` family): the rail's ADD button opens it at the canvas centre, a
    /// finger opens it with the empty-canvas long-press, and both ride this path.
    pub fn browser_open_at(&mut self, anchor: Vec2, view: Rect) -> Vec<CanvasEvent> {
        let spawn_world = self.camera.to_world(anchor, view);
        self.browser = Some(BrowserState::open(self.catalog.clone(), anchor, spawn_world));
        vec![CanvasEvent::Browser(true)]
    }

    /// Spawn a catalogued module by id at a world position — the dock card's door: the SAME
    /// snap/cascade/commit path as the browser's row tap (`spawn_node`), undoable and
    /// ledger-marked like every edit. Refuses in words when the id is not installed — #58
    /// reaches the dock too.
    pub fn spawn_module(
        &mut self,
        graph: &mut Graph,
        module_id: &str,
        world: Vec2,
    ) -> Vec<CanvasEvent> {
        let Some(item) = self.catalog.iter().find(|i| i.spec.module_id == module_id) else {
            return vec![CanvasEvent::Refused(format!(
                "module `{module_id}` is not installed — the dock cannot offer what does not                  exist (defect #58)"
            ))];
        };
        let spec = item.spec.clone();
        self.spawn_node(graph, spec, world)
    }

    /// The catalogue the browser ranks — the dock's card grid reads the SAME list: one source,
    /// so the dock and the sheet cannot disagree about what is installed.
    #[must_use]
    pub fn catalog(&self) -> &[BrowserItem] {
        &self.catalog
    }

    /// Edit one param through the model — clamped/snapped, one undo step, and marked on the
    /// live-sync ledger (`take_patch_changes`). The documented headless/driver path, mirroring
    /// [`Self::browser_set_query`]: the window path arrives here through a slider drag
    /// (`apply_param` with coalescing); drivers and smokes call this directly and go through
    /// the SAME door the finger does.
    pub fn param_edit(
        &mut self,
        graph: &mut Graph,
        node: NodeId,
        index: usize,
        value: f32,
    ) -> Vec<CanvasEvent> {
        self.apply_param(graph, node, index, value, false)
    }

    /// Resolve and commit a CONTROL wire (operator ruling 2026-10-01 r3): a cv output onto a
    /// float parameter — the same `connect::resolve_param` verdicts and the same `commit`
    /// door the drop path rides. Documented headless/driver path, the `connect_ports`
    /// sibling; the live-sync ledger marks it exactly as the drag would (structural).
    pub fn connect_param(
        &mut self,
        graph: &mut Graph,
        from: PortRef,
        dst: NodeId,
        param: usize,
    ) -> Vec<CanvasEvent> {
        match connect::resolve_param(graph, from, dst, param) {
            ConnectOutcome::Connected { op, replaced, .. } => {
                self.commit(op);
                let mut ev = vec![CanvasEvent::Applied("control connected".to_string())];
                if replaced {
                    ev.push(CanvasEvent::Note(
                        "replaced the modulation on that parameter".to_string(),
                    ));
                }
                ev
            },
            ConnectOutcome::Refused(r) => vec![CanvasEvent::Refused(r.reason)],
        }
    }

    /// Resolve and commit a wire between two ports — the same `connect::resolve` verdicts and
    /// the same `commit` door the drag path (`finish_wire`) rides, without the layout and
    /// pending-wire machinery only a finger can supply. Documented headless/driver path; the
    /// live-sync ledger marks it exactly as the drag would.
    pub fn connect_ports(
        &mut self,
        graph: &mut Graph,
        from: PortRef,
        to: PortRef,
        ctx: &ConnectContext<'_>,
    ) -> Vec<CanvasEvent> {
        // The junction bus (r3): a mult dot's role is the wire's orientation, not the
        // manifest's static direction — pass the pair through as given and let the bus rules
        // judge it in words.
        let from_mult =
            graph.node(from.node).is_some_and(|n| n.spec.module_id == crate::canvas::MULT_ID);
        let to_mult =
            graph.node(to.node).is_some_and(|n| n.spec.module_id == crate::canvas::MULT_ID);
        if from_mult || to_mult {
            return match connect::resolve(graph, from, to, ctx) {
                ConnectOutcome::Connected { op, replaced, .. } => {
                    self.commit(op);
                    let mut ev = vec![CanvasEvent::Applied("connect".to_string())];
                    if replaced {
                        ev.push(CanvasEvent::Note(
                            "replaced the wire on that single input".to_string(),
                        ));
                    }
                    ev
                },
                ConnectOutcome::Refused(r) => vec![CanvasEvent::Refused(r.reason)],
            };
        }
        // Normalise orientation out→in regardless of argument order, as the drag path does.
        let from_dir = graph.port(from).map(|pt| pt.direction).unwrap_or(Direction::Out);
        let to_dir = graph.port(to).map(|pt| pt.direction).unwrap_or(Direction::In);
        let (src, dst) = match (from_dir, to_dir) {
            (Direction::Out, Direction::In) => (from, to),
            (Direction::In, Direction::Out) => (to, from),
            _ => {
                return vec![CanvasEvent::Refused(
                    "a wire runs from an output to an input".to_string(),
                )];
            },
        };
        match connect::resolve(graph, src, dst, ctx) {
            ConnectOutcome::Connected { op, conversion, replaced, adapter } => {
                self.commit(op);
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

    /// Replace the sheet's buffer — the documented headless path, mirroring
    /// [`Self::browser_set_query`].
    pub fn rename_set_text(&mut self, text: &str) {
        if let Some(r) = &mut self.rename {
            r.entry.set_text(text);
        }
    }

    /// Commit the sheet (the shell's Enter): an EMPTY buffer clears the rename — the node falls
    /// back to its module's display name, which is the hint row's stated rule — otherwise the
    /// typed name is set. The op is undoable like every edit; an unchanged commit says so in
    /// words and leaves the history alone (an undo step that undoes nothing is a lie).
    pub fn rename_commit(&mut self, graph: &mut Graph) -> Vec<CanvasEvent> {
        let Some(r) = self.rename.take() else { return Vec::new() };
        // The buffer is what it opened with: nothing was typed, nothing is recorded. This catches
        // the case `op_rename`'s equality cannot — committing the MODULE DEFAULT verbatim on a
        // node with no custom name would otherwise pin a rename that changes no visible name.
        if r.entry.is_unchanged() {
            return vec![
                CanvasEvent::Rename(false),
                CanvasEvent::Note("the name is unchanged".to_string()),
            ];
        }
        let text = r.entry.committed();
        let to = if text.is_empty() { None } else { Some(text) };
        let msg = match &to {
            Some(t) => format!("renamed to `{t}`"),
            None => "name cleared — the module default is back".to_string(),
        };
        match graph.op_rename(r.node, to) {
            Some(op) => {
                self.commit(op);
                vec![CanvasEvent::Rename(false), CanvasEvent::Applied(msg)]
            },
            None => {
                vec![CanvasEvent::Rename(false), CanvasEvent::Note("the name is unchanged".into())]
            },
        }
    }

    /// Cancel the sheet (Escape, an outside tap, or a louder gesture opening over it): the
    /// buffer is dropped and the name untouched — stated, never silent.
    pub fn rename_cancel(&mut self) -> Vec<CanvasEvent> {
        if self.rename.take().is_some() {
            vec![
                CanvasEvent::Rename(false),
                CanvasEvent::Note("rename cancelled — the name is unchanged".to_string()),
            ]
        } else {
            Vec::new()
        }
    }

    /// Whether a screen point lands inside the open rename sheet — the modal's capture test, on
    /// the sheet's own geometry (one clamping rule, shared with the menu and the browser).
    #[must_use]
    pub fn rename_contains(&self, pos: Vec2, view: Rect) -> bool {
        let Some(r) = &self.rename else { return false };
        let (max_w, _) = crate::canvas::browser::caps(view);
        r.contains(pos, view, max_w)
    }

    /// Store this frame's inspector geometry (the shell computes it from the selection and the
    /// shell layout's inspector rect). The layout carries the CLAMPED offset, so storing it back
    /// here is what keeps the request and the geometry from drifting apart.
    pub fn set_inspector(&mut self, il: Option<InspectorLayout>) {
        match &il {
            Some(l) => {
                if self.insp_node != Some(l.node) {
                    // A different panel: the probe marker is the OLD node's display state and
                    // does not survive the selection change (D3 — reset, not carried).
                    self.response_marker = None;
                    self.resp_axes = None;
                }
                self.insp_node = Some(l.node);
                self.insp_scroll = l.scroll;
            },
            None => {
                self.insp_node = None;
                self.insp_scroll = 0.0;
                self.response_marker = None;
                self.resp_axes = None;
            },
        }
        self.inspector = il;
    }

    /// Store the response axes the shell computed this frame's curve with (`None` while the
    /// inspected module declares no curve). The marker drag maps x → frequency through THESE
    /// axes — the same log scale the painter plots, so the readout and the pixels cannot
    /// disagree. Called per frame beside [`Self::set_inspector`].
    pub fn set_response_axes(&mut self, entry: Option<(NodeId, crate::canvas::response::Axes)>) {
        self.resp_axes = entry;
    }

    /// The probe marker's frequency for `node`, clamped into the live axes' range — `None`
    /// while no marker is placed or it belongs to a different node (the reset-on-selection-
    /// change rule, read where it matters).
    #[must_use]
    pub fn response_marker(&self, node: NodeId) -> Option<f64> {
        let (m_node, hz) = self.response_marker?;
        if m_node != node {
            return None;
        }
        Some(match self.resp_axes {
            Some((a_node, axes)) if a_node == node => axes.clamp_freq(hz),
            _ => hz,
        })
    }

    /// Place the probe marker (the tap-to-place and drag paths' single door): clamped into the
    /// declared frequency range, a non-finite ask reading the floor. Sets NO param, pushes NO
    /// history, notes NOTHING on the live-sync ledger — the marker is a reading (D3).
    pub fn set_response_marker(&mut self, node: NodeId, hz: f64) {
        use crate::canvas::response::{FREQ_MAX_HZ, FREQ_MIN_HZ};
        let hz = if hz.is_finite() { hz.clamp(FREQ_MIN_HZ, FREQ_MAX_HZ) } else { FREQ_MIN_HZ };
        self.response_marker = Some((node, hz));
    }

    /// The frequency a screen x inside the plot well probes — the marker mapping's one door
    /// (drag start, drag update and tap-to-place all read it). `None` when no curve axes are
    /// live for `node` or the layout went stale: a probe with nothing to probe is refused,
    /// never guessed.
    fn marker_freq_at(&self, node: NodeId, x: f32) -> Option<f64> {
        let (a_node, axes) = self.resp_axes?;
        if a_node != node {
            return None;
        }
        let plot = self.inspector.as_ref()?.plot?;
        Some(axes.freq_of_x(plot, x))
    }

    /// The scroll offset to lay `node`'s panel out at: the stored one when it is the same node
    /// still being inspected, else zero — selecting a different node starts at its first row.
    #[must_use]
    pub fn inspector_scroll(&self, node: NodeId) -> f32 {
        if self.insp_node == Some(node) {
            self.insp_scroll
        } else {
            0.0
        }
    }

    /// Scroll the inspector by `delta_y` screen px (content follows the fingers: a downward pan
    /// reveals earlier rows). The offset is clamped by the panel geometry every call.
    ///
    /// Logging follows the camera-pan precedent — continuous direct manipulation is its own
    /// feedback (the rows move, the thumb tracks), so the panel speaks only at the DISCRETE
    /// moments: when the scroll hits an end and further panning does nothing. An input that did
    /// nothing stays explainable; an input that is visibly working does not narrate itself.
    /// A panel with `max_scroll == 0` shows every row and no thumb — visibly nothing to scroll.
    pub fn scroll_inspector(&mut self, delta_y: f32) -> Vec<CanvasEvent> {
        let Some((max_scroll, n)) =
            self.inspector.as_ref().map(|il| (il.max_scroll, il.rows.len()))
        else {
            return Vec::new();
        };
        if max_scroll <= 0.0 || n == 0 {
            return Vec::new();
        }
        let before = self.insp_scroll;
        let after = (before - delta_y).clamp(0.0, max_scroll);
        self.insp_scroll = after;
        let hit_top = after <= f32::EPSILON && before > f32::EPSILON;
        let hit_end = after >= max_scroll - f32::EPSILON && before < max_scroll - f32::EPSILON;
        if hit_top {
            vec![CanvasEvent::Note("inspector: scrolled to the first row".to_string())]
        } else if hit_end {
            vec![CanvasEvent::Note(format!("inspector: scrolled to the last row ({n} rows)"))]
        } else {
            Vec::new()
        }
    }

    /// The inspector geometry, for the painter.
    #[must_use]
    pub fn inspector(&self) -> Option<&InspectorLayout> {
        self.inspector.as_ref()
    }

    /// Zoom to fit the graph's bounds (the toolbar's FIT — the DoubleTap intent's door, named
    /// for the button too).
    pub fn zoom_fit(&mut self, graph: &Graph, view: Rect) {
        self.zoom_to_fit(graph, view);
    }

    /// The camera home (toolbar RESET): default origin and zoom, the patch where the patch
    /// grid starts.
    pub fn camera_home(&mut self) {
        let cam = crate::canvas::camera::Camera::new();
        self.camera = cam;
        let _ = self.lod_events();
    }

    /// One zoom step about the view centre (the toolbar's − / +).
    pub fn zoom_step(&mut self, factor: f32, view: Rect) {
        self.camera.zoom_about(factor, view.center(), view);
        let _ = self.lod_events();
    }

    /// Grid-arrange the whole graph as ONE undo step (the toolbar's ARRANGE, increment 6):
    /// node-id order into rows of four on the world grid. Positions only — a Batch of
    /// `MoveNode`s, so the live engine hears nothing (the ledger's move-rule). `None`-free:
    /// an already-arranged graph commits nothing and says so.
    pub fn arrange_graph(&mut self, graph: &mut Graph) -> Vec<CanvasEvent> {
        use crate::tokens::{LAYOUT_CANVAS_NODE_WIDTH_DEFAULT, LAYOUT_SPACE_8, LAYOUT_SPACE_9};
        let mut moves = Vec::new();
        let ids: Vec<NodeId> = graph.nodes().iter().map(|n| n.id).collect();
        for (i, id) in ids.iter().enumerate() {
            let Some(from) = graph.node(*id).map(|n| n.pos) else { continue };
            let col = (i % 4) as f32;
            let row = (i / 4) as f32;
            let to = Vec2::new(
                col * (LAYOUT_CANVAS_NODE_WIDTH_DEFAULT as f32 + LAYOUT_SPACE_8 as f32),
                row * (LAYOUT_SPACE_9 as f32 * 4.0),
            );
            if (from.x - to.x).abs() > f32::EPSILON || (from.y - to.y).abs() > f32::EPSILON {
                moves.push(Op::MoveNode { id: *id, from, to });
            }
        }
        if moves.is_empty() {
            return vec![CanvasEvent::Note("arrange: the patch is already on the grid".into())];
        }
        let n = moves.len();
        let op = Op::Batch(moves);
        graph.apply(&op);
        self.commit(op);
        vec![CanvasEvent::Applied(format!("arranged {n} node(s) on the grid"))]
    }

    /// The wire-draw style (increment 6 toolbar): smooth béziers or straight runs. Display
    /// state like the camera — not undoable, not the engine's business.
    pub fn toggle_wire_style(&mut self) -> &'static str {
        self.wire_straight = !self.wire_straight;
        if self.wire_straight {
            "straight"
        } else {
            "smooth"
        }
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
            graph
                .wires()
                .iter()
                .any(|w| w.param.is_none() && (w.from.node == id || w.to.node == id))
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
            GestureIntent::Pan { delta, center } => self.pan(delta, center),
            GestureIntent::Zoom { factor, center } => {
                if self.over_inspector(center) {
                    // The panel owns the gesture: it has no zoom of its own, and the canvas
                    // BEHIND it must not move because fingers pinched over the panel. (This is
                    // also what keeps the sequential-contact span wobble of a two-finger drag —
                    // the recogniser processes contacts one event at a time, so a straight
                    // vertical drag momentarily reads as a pinch — off the camera AND off the
                    // panel's scroll.) The MOUSE WHEEL never arrives here over a panel: the
                    // window adapter routes it to the panel's own `Pan` scroll (operator ruling
                    // 2026-10-01: wheel = canvas zoom, and a panel keeps its scroll).
                    Vec::new()
                } else {
                    self.camera.zoom_about(factor, center, view);
                    self.lod_events()
                }
            },
            GestureIntent::Undo => self.undo(graph),
            GestureIntent::Delete => self.delete_selection(graph),
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

    /// The two-finger pan, routed by WHERE it happens (increment 5): over the inspector panel it
    /// scrolls the panel — one finger on a row is a slider edit, so the panel's scroll rides the
    /// two-finger gesture — and anywhere else it pans the camera, exactly as before. The
    /// recogniser's `center` is what tells the two apart, the same reason `Zoom` has carried its
    /// centre since WO-012.
    fn pan(&mut self, delta: Vec2, center: Vec2) -> Vec<CanvasEvent> {
        if self.over_inspector(center) {
            return self.scroll_inspector(delta.y);
        }
        self.camera.pan_by_screen(delta);
        self.lod_events()
    }

    /// Whether a two-finger gesture's centre lands on the inspector panel — the routing rule
    /// that gives the panel its scroll (increment 5): a gesture OVER a surface belongs to that
    /// surface, so the pan scrolls the rows and the pinch/rotate components are declined there
    /// rather than moving the canvas underneath the user's fingers.
    fn over_inspector(&self, center: Vec2) -> bool {
        self.inspector.as_ref().is_some_and(|il| il.rect.contains(center))
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
            Some(op) => {
                // The inverse is an edit like any other: it goes through the same ledger, so a
                // live session re-syncs on undo exactly as it does on the original op.
                self.note_patch(&op);
                vec![CanvasEvent::Undo(op.label().to_string())]
            },
            None => vec![CanvasEvent::Refused("nothing to undo".to_string())],
        }
    }

    fn redo(&mut self, graph: &mut Graph) -> Vec<CanvasEvent> {
        match self.history.redo(graph) {
            Some(op) => {
                self.note_patch(&op);
                vec![CanvasEvent::Redo(op.label().to_string())]
            },
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
        // The open rename sheet is the DEEPEST modal (it is opened FROM the menu): a tap inside
        // it is answered in words — the keyboard owns the buffer — and a tap outside cancels,
        // stated, never silent.
        if self.rename.is_some() {
            if self.rename_contains(pos, view) {
                return vec![CanvasEvent::Note(
                    "the rename field takes the keyboard — type, then ENTER".to_string(),
                )];
            }
            return self.rename_cancel();
        }

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

        // The response plot under the finger places the probe marker where you tapped
        // (tap-to-place — the slider's tap-to-set courtesy, on a reading instead of a param).
        if let Some(ev) = self.activate_response_plot(pos) {
            return ev;
        }

        // An inspector slider under the finger sets the value where you tapped (tap-to-set;
        // a drag from here adjusts continuously).
        if let Some(ev) = self.activate_inspector(graph, pos) {
            return ev;
        }

        let lod = self.camera.lod();
        match layout::hit_test(layout, pos, lod) {
            Hit::Step(node, step) => {
                // A step button: the tap TOGGLES that step's bit in the pattern mask — the
                // button is the program, one undo step per flip (operator ruling r3).
                self.selection.clear();
                self.selection.nodes.insert(node);
                self.flip_step(graph, node, step)
            },
            Hit::Key(node, key) => {
                // A keyboard key (operator round 4, D11): the tap flips that pitch class's
                // membership in the CUSTOM scale — the key is the scale, one undo step per
                // flip, through the param door (the Step button's rule, on twelve keys).
                self.selection.clear();
                self.selection.nodes.insert(node);
                self.flip_key(graph, node, key)
            },
            Hit::Param(node, index, track) => {
                // The node card's own slider (increment 6): tap-to-set, the inspector's rule —
                // the row under the finger is the row you edit, and one op serves both surfaces.
                // A BINARY row is a toggle button (operator ruling 2026-10-01): the tap FLIPS
                // it, x is irrelevant.
                self.selection.clear();
                self.selection.nodes.insert(node);
                let desc = graph.node(node).and_then(|n| n.spec.params.get(index).cloned());
                match desc {
                    Some(d) => {
                        let v = if inspector::is_binary(&d) {
                            let cur =
                                graph.node(node).and_then(|n| n.param_value(index)).unwrap_or(0.0);
                            flipped(&d, cur)
                        } else {
                            inspector::value_from_x(&d, track, pos.x)
                        };
                        self.apply_param(graph, node, index, v, false)
                    },
                    None => vec![CanvasEvent::Refused("no such parameter".to_string())],
                }
            },
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
            Hit::WireTrim(id) => {
                // The cable node itself (operator round 4, D15): the tap REMOVES it — the
                // vocabulary the insert note taught — and the wire runs clean again. One
                // undo step, and the engine hears a re-stage (trims are structural).
                self.selection.clear();
                self.selection.wires.insert(id);
                let mut ev = vec![CanvasEvent::Selection(1)];
                ev.extend(self.apply_trim(graph, id, None, false));
                ev
            },
            Hit::Wire(id) | Hit::WireEnd(id, _) => {
                self.selection.clear();
                self.selection.wires.insert(id);
                // The cable node's INSERT door (operator round 4, D15): a tap on a CLEAN
                // wire inside the hover ghost's capture puts the node there — at identity,
                // so the handle appears and nothing is heard — and the note teaches the
                // vocabulary the handle's own tap will rely on (drag to edit, tap the node
                // to remove). Anywhere else on the wire, the tap only selects, as ever.
                let ghost = graph.wire(id).is_some_and(|w| w.trim.is_none())
                    && layout.wires.iter().any(|wl| {
                        wl.id == id
                            && wl.trim_point.distance(pos)
                                <= LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32
                    });
                let mut ev = vec![CanvasEvent::Selection(1)];
                if ghost {
                    ev.extend(self.apply_trim(graph, id, Some(WireTrim::identity()), false));
                    ev.push(CanvasEvent::Note(
                        "cable node inserted — drag it (up/down = amp, left/right = offset); \
                         tap the node to remove it"
                            .to_string(),
                    ));
                }
                ev
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
    /// spawn never lands exactly under the first). The permanent Main Out is refused: the user
    /// never creates it — it is always already on the canvas (operator ruling 2026-10-01).
    fn spawn_node(
        &mut self,
        graph: &mut Graph,
        spec: crate::canvas::model::NodeSpec,
        world: Vec2,
    ) -> Vec<CanvasEvent> {
        if spec.module_id == crate::canvas::OUT_MAIN_ID {
            return vec![CanvasEvent::Refused(
                "Main Out is permanent — it is already on the canvas and cannot be added again"
                    .to_string(),
            )];
        }
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
        self.commit(op);
        self.selection.clear();
        self.selection.nodes.insert(new_id);
        vec![
            CanvasEvent::Browser(false),
            CanvasEvent::Applied(format!("added {name}")),
            CanvasEvent::Selection(1),
        ]
    }

    /// Tap inside the response-plot well: place the probe marker at the tapped frequency.
    /// Returns `None` when the tap is not in the plot well (the caller continues to the slider
    /// rows and then the canvas). A no-curve well returns `None` too — the shell routes those
    /// taps to the chrome path, and a direct intent gets the well's own words back.
    fn activate_response_plot(&mut self, pos: Vec2) -> Option<Vec<CanvasEvent>> {
        let (node_id, in_plot) = self
            .inspector
            .as_ref()
            .map(|il| (il.node, il.plot.is_some_and(|r| r.contains(pos))))?;
        if !in_plot {
            return None;
        }
        let hz = self.marker_freq_at(node_id, pos.x)?;
        self.set_response_marker(node_id, hz);
        Some(vec![CanvasEvent::Note(format!(
            "response marker: {} (a reading — cutoff stays the slider's)",
            crate::canvas::response::format_hz(hz)
        ))])
    }

    /// Tap on an inspector row: select the node and set the parameter at the tapped x — or FLIP
    /// it when the row is a binary toggle (operator ruling 2026-10-01: binary settings are
    /// toggle buttons, never sliders). Returns `None` when the tap is not on the inspector (the
    /// caller continues to the canvas).
    fn activate_inspector(&mut self, graph: &mut Graph, pos: Vec2) -> Option<Vec<CanvasEvent>> {
        // A step button inside the pattern row wins over the row's x-mapping (r3).
        if let Some((node_id, step)) = self.inspector.as_ref().and_then(|il| {
            il.row_at(pos).and_then(|i| {
                il.rows[i].steps.iter().position(|c| c.contains(pos)).map(|k| (il.node, k))
            })
        }) {
            self.selection.clear();
            self.selection.nodes.insert(node_id);
            return Some(self.flip_step(graph, node_id, step));
        }
        let (node_id, index, editable, toggle, track) = self.inspector.as_ref().and_then(|il| {
            il.row_at(pos).map(|i| {
                (
                    il.node,
                    il.rows[i].index,
                    il.rows[i].editable,
                    il.rows[i].toggle,
                    il.rows[i].track,
                )
            })
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
        let v = if toggle {
            let cur = graph.node(node_id).and_then(|n| n.param_value(index)).unwrap_or(0.0);
            flipped(&desc, cur)
        } else {
            inspector::value_from_x(&desc, track, pos.x)
        };
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
                        // The merged value is a fresh edit the ledger has not seen (the earlier
                        // take may already have carried the first waypoint away) — note it, or
                        // the live engine keeps playing the drag's first frame.
                        self.note_patch(&op);
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
        self.commit(op);
        vec![CanvasEvent::Applied(text)]
    }

    /// One wire-trim edit through the model (which clamps/sanitises), with history — the
    /// cable node's single door (operator round 4, D15). `coalesce` is true only for the
    /// updates *inside* an open node drag whose first change already pushed: those replace
    /// the open entry keeping its ORIGINAL `from`, so one drag is one undo step restoring
    /// the trim the finger found, and the log stays quiet until the lift (`apply_param`'s
    /// coalescing, mirrored exactly). `Some(identity)` is the tap-insert on the hover
    /// ghost; `None` is the tap-remove on the handle itself.
    fn apply_trim(
        &mut self,
        graph: &mut Graph,
        wire: WireId,
        to: Option<WireTrim>,
        coalesce: bool,
    ) -> Vec<CanvasEvent> {
        let Some(op) = graph.op_set_trim(wire, to) else {
            return Vec::new(); // no change (or no such wire): silent, the handle says it
        };
        if coalesce {
            if let Some(Op::SetTrim { wire: pw, from: pfrom, .. }) = self.history.top() {
                if *pw == wire {
                    // Merge into the open entry — keeping its ORIGINAL `from`, so one undo
                    // restores the trim the finger found, not the drag's first waypoint.
                    if let Op::SetTrim { to, .. } = &op {
                        self.history.replace_top(Op::SetTrim { wire, from: *pfrom, to: *to });
                        // The merged value is a fresh edit the ledger has not seen (the
                        // earlier take may already have carried the first waypoint away) —
                        // note it, or the live engine keeps playing the drag's first frame.
                        self.note_patch(&op);
                    }
                    return Vec::new();
                }
            }
        }
        let text = match &op {
            Op::SetTrim { to: Some(t), .. } => {
                format!("wire trim: amp {:.2} · offset {:.2}", t.amp, t.offset)
            },
            Op::SetTrim { to: None, .. } => "wire trim removed".to_string(),
            _ => "wire trim".to_string(),
        };
        self.commit(op);
        vec![CanvasEvent::Applied(text)]
    }

    /// Toggle one step button of `mod/seq`'s pattern (operator ruling 2026-10-01 r3): the
    /// mask param's bit `step` flips, through the same op door as every param edit — one
    /// undo step per flip, the live ring hears it like any edit.
    fn flip_step(&mut self, graph: &mut Graph, node: NodeId, step: usize) -> Vec<CanvasEvent> {
        let idx = match graph
            .node(node)
            .and_then(|n| n.spec.params.iter().position(|d| d.id == "pattern"))
        {
            Some(i) => i,
            None => return vec![CanvasEvent::Refused("that node has no step pattern".to_string())],
        };
        let cur = graph.node(node).and_then(|n| n.param_value(idx)).unwrap_or(0.0);
        let bit = step_mask_bit(step);
        let masked = cur as i64 & bit as i64;
        let next = if masked != 0 { (cur as i64) - bit as i64 } else { (cur as i64) | bit as i64 };
        self.apply_param(graph, node, idx, next as f32, false)
    }

    /// Toggle one key of `util/quant`'s keyboard (operator round 4, D11): the key's bit in
    /// the CUSTOM scale's `custom-mask` flips, through the same param door as every edit —
    /// one undo step per flip, the live ring hears it like any edit. The CUSTOM-MODE GATE
    /// comes first: the keyboard edits the Custom scale, so a preset scale refuses in words
    /// with the remedy — never a silent mask edit the scale list would ignore.
    fn flip_key(&mut self, graph: &mut Graph, node: NodeId, key: usize) -> Vec<CanvasEvent> {
        // The gate (D11): scale value 0 IS Custom; any other value is a preset the mask does
        // not voice, so editing it would be a lie the user could act on.
        let preset = graph
            .node(node)
            .and_then(|n| n.spec.params.iter().position(|d| d.id == "scale"))
            .and_then(|si| graph.node(node).and_then(|n| n.param_value(si)));
        if preset.is_some_and(|v| v.round() != 0.0) {
            return vec![CanvasEvent::Refused(
                "the keyboard edits the CUSTOM scale — pick Custom from the scale list first"
                    .to_string(),
            )];
        }
        let idx = match graph
            .node(node)
            .and_then(|n| n.spec.params.iter().position(|d| d.id == "custom-mask"))
        {
            Some(i) => i,
            None => {
                return vec![CanvasEvent::Refused("that node has no scale keyboard".to_string())]
            },
        };
        let cur = graph.node(node).and_then(|n| n.param_value(idx)).unwrap_or(0.0);
        let bit = key_mask_bit(key);
        let masked = cur as i64 & bit as i64;
        let next = if masked != 0 { (cur as i64) - bit as i64 } else { (cur as i64) | bit as i64 };
        self.apply_param(graph, node, idx, next as f32, false)
    }

    fn open_menu(&mut self, graph: &Graph, pos: Vec2, layout: &CanvasLayout) -> Vec<CanvasEvent> {
        // A long-press over an open browser closes it and opens the menu where you pressed —
        // the menu is the deeper modal (it can re-open the browser). An open rename sheet
        // cancels the same way: the long-press is a louder question than the one being typed.
        let mut ev = self.rename_cancel();
        if self.browser.is_some() {
            self.browser = None;
            ev.push(CanvasEvent::Browser(false));
        }
        let lod = self.camera.lod();
        let target = match layout::hit_test(layout, pos, lod) {
            // A long-press on a key targets its NODE (the keyboard is the node's surface,
            // the step strip's rule).
            Hit::Node(id) | Hit::Step(id, _) | Hit::Key(id, _) => {
                self.selection.clear();
                self.selection.nodes.insert(id);
                MenuTarget::Node(id)
            },
            // A long-press on a cable node targets its WIRE: the menu's DELETE WIRE row
            // removes the wire and its trim record together (the per-wire ruling, D15).
            Hit::Wire(id) | Hit::WireEnd(id, _) | Hit::WireTrim(id) => {
                self.selection.clear();
                self.selection.wires.insert(id);
                MenuTarget::Wire(id)
            },
            Hit::Port(pref, _) => {
                self.selection.clear();
                self.selection.nodes.insert(pref.node);
                MenuTarget::Node(pref.node)
            },
            Hit::Param(node, _, _) => {
                self.selection.clear();
                self.selection.nodes.insert(node);
                MenuTarget::Node(node)
            },
            Hit::Empty => MenuTarget::Empty,
        };
        let rows: Vec<MenuRow> = match target {
            MenuTarget::Node(id) => {
                let flags = graph.node(id).map(|n| n.flags).unwrap_or_default();
                vec![
                    MenuRow::row(MenuAction::Rename, "RENAME", true),
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
            (MenuTarget::Node(id), MenuAction::Rename) => self.rename_open(graph, id, anchor),
            (MenuTarget::Node(id), MenuAction::Duplicate) => self.duplicate(graph, id),
            (MenuTarget::Node(id), MenuAction::SetMaster) => {
                self.master = Some(id);
                // Not an `Op` (the master designation carries no undo entry), but the live
                // engine hears it: which node feeds the device is structural. The session
                // re-stages with the new master on its next sync.
                self.patch_changes.structural = true;
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
        // The permanent Main Out has no copy (operator ruling 2026-10-01): duplicating it
        // would be creating it, which the user never does.
        if src.spec.module_id == crate::canvas::OUT_MAIN_ID {
            return vec![CanvasEvent::Refused(
                "Main Out is permanent — it cannot be duplicated".to_string(),
            )];
        }
        let grid = LAYOUT_CANVAS_SNAP as f32;
        let pos = snap(Vec2::new(src.pos.x + grid * 3.0, src.pos.y + grid * 3.0), grid);
        let op = graph.op_add_node(src.spec.clone(), pos);
        let new_id = match &op {
            Op::AddNode(n) => n.id,
            _ => return vec![CanvasEvent::Refused("duplicate failed".to_string())],
        };
        self.commit(op);
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
                self.commit(op);
                vec![CanvasEvent::Applied(label.to_string())]
            },
            None => vec![CanvasEvent::Note("no change".to_string())],
        }
    }

    fn delete_node(&mut self, graph: &mut Graph, id: NodeId) -> Vec<CanvasEvent> {
        self.remove_node_spliced(graph, id)
    }

    /// Remove one node AND KEEP THE CHAIN CONNECTED (operator ruling 2026-10-01): a deleted
    /// module used to leave its source dangling and its destinations silent. Now the wires
    /// that fed the node's inputs are SPLICED onto the wires its outputs fed — paired in port
    /// order (first input's source to first output's destination, and so on), each splice run
    /// through the same connection verdict a hand-drawn wire gets (`connect::resolve`: type,
    /// range, cycle, single-input replacement), so an incompatible pair is skipped, not
    /// forced. The removal and every accepted splice ride ONE [`Op::Batch`] into history: one
    /// undo restores the node, its old wires, and removes the splices — the deletion was one
    /// action, so it undoes as one. Protections speak first: the permanent Main Out and locked
    /// nodes refuse in words and stay.
    fn remove_node_spliced(&mut self, graph: &mut Graph, id: NodeId) -> Vec<CanvasEvent> {
        // The permanent node (operator ruling 2026-10-01): Main Out is where the listener
        // lives — the user never creates it and never deletes it. The refusal is a sentence
        // with the reason, like every other voice.
        if graph.node(id).is_some_and(|n| n.spec.module_id == crate::canvas::OUT_MAIN_ID) {
            return vec![CanvasEvent::Refused(
                "Main Out is permanent — it is the listener's output and cannot be deleted"
                    .to_string(),
            )];
        }
        let locked = graph.node(id).map(|n| n.flags.locked).unwrap_or(false);
        if locked {
            return vec![CanvasEvent::Refused(
                "node is locked — unlock it from the long-press menu first".to_string(),
            )];
        }
        // The splice pairs, read BEFORE the removal: inputs by input index, outputs by output
        // index, paired positionally.
        let mut ins: Vec<(PortRef, PortRef)> = graph
            .wires()
            .iter()
            .filter(|w| w.param.is_none() && w.to.node == id)
            .map(|w| (w.to, w.from))
            .collect();
        ins.sort_by_key(|(dst, _)| dst.index);
        let mut outs: Vec<PortRef> = graph
            .wires()
            .iter()
            .filter(|w| w.param.is_none() && w.from.node == id)
            .map(|w| w.to)
            .collect();
        outs.sort_by_key(|d| d.index);
        let pairs: Vec<(PortRef, PortRef)> = ins.iter().map(|(_, src)| *src).zip(outs).collect();
        let Some(rm) = graph.op_remove_node(id) else {
            return vec![CanvasEvent::Refused("no such node".to_string())];
        };
        let mut ops = vec![rm];
        let ctx = ConnectContext::no_adapters(Phase::Zero);
        let mut spliced = 0usize;
        for (src, dst) in pairs {
            if let ConnectOutcome::Connected { op, .. } = connect::resolve(graph, src, dst, &ctx) {
                ops.push(op);
                spliced += 1;
            }
        }
        self.commit(Op::Batch(ops));
        self.selection.clear();
        let mut ev = vec![CanvasEvent::Applied("delete node".to_string())];
        if spliced > 0 {
            ev.push(CanvasEvent::Note(format!(
                "chain kept: {spliced} wire(s) spliced past the deleted node"
            )));
        }
        ev
    }

    fn delete_wire(&mut self, graph: &mut Graph, id: WireId) -> Vec<CanvasEvent> {
        match graph.op_remove_wire(id) {
            Some(op) => {
                self.commit(op);
                self.selection.clear();
                vec![CanvasEvent::Applied("delete wire".to_string())]
            },
            None => vec![CanvasEvent::Refused("no such wire".to_string())],
        }
    }

    /// The DELETE key's action (operator ruling 2026-10-01): remove the current selection —
    /// wires first, then nodes — each through the same door (and the same history) the
    /// long-press menu's DELETE rides. Protected nodes refuse in words and survive: a locked
    /// node stays locked, and the permanent Main Out stays on the canvas even in a select-all
    /// sweep. An empty selection is a refusal with the remedy, never a silence.
    pub fn delete_selection(&mut self, graph: &mut Graph) -> Vec<CanvasEvent> {
        if self.rename.is_some() {
            return vec![CanvasEvent::Refused(
                "the rename field has the keyboard — ENTER commits, ESCAPE cancels".to_string(),
            )];
        }
        if self.browser.is_some() || self.menu.is_some() {
            return vec![CanvasEvent::Refused(
                "a sheet is open — close it first, then DELETE removes the selection".to_string(),
            )];
        }
        if self.selection.is_empty() {
            return vec![CanvasEvent::Refused(
                "nothing selected — tap a node or a wire, then press DELETE".to_string(),
            )];
        }
        let wires: Vec<WireId> = self.selection.wires.iter().copied().collect();
        let nodes: Vec<NodeId> = self.selection.nodes.iter().copied().collect();
        let mut ev = Vec::new();
        let mut deleted = 0usize;
        for w in wires {
            match graph.op_remove_wire(w) {
                Some(op) => {
                    self.commit(op);
                    deleted += 1;
                },
                None => ev.push(CanvasEvent::Refused("no such wire".to_string())),
            }
        }

        for n in nodes {
            // The same splicing door the menu's DELETE rides (chain stays connected), and the
            // same protections: Main Out and locked nodes refuse in words and survive.
            let before = graph.node_count();
            let mut one = self.remove_node_spliced(graph, n);
            if graph.node_count() == before {
                // refused (permanent / locked / absent) — keep the refusal, not the count
                ev.append(&mut one);
            } else {
                deleted += 1;
                // keep only the splice note; the per-node "delete node" word would repeat
                for e in one {
                    if matches!(e, CanvasEvent::Note(_)) {
                        ev.push(e);
                    }
                }
            }
        }
        self.selection.clear();
        if deleted > 0 {
            ev.insert(
                0,
                CanvasEvent::Applied(format!(
                    "deleted {deleted} {}",
                    if deleted == 1 { "item" } else { "items" }
                )),
            );
        }
        ev.push(CanvasEvent::Selection(0));
        ev
    }

    fn drag_start(
        &mut self,
        graph: &mut Graph,
        pos: Vec2,
        layout: &CanvasLayout,
    ) -> Vec<CanvasEvent> {
        // An open menu, browser or rename sheet is dismissed by a drag that starts outside it.
        let mut ev = self.rename_cancel();
        if self.browser.is_some() {
            self.browser = None;
            ev.push(CanvasEvent::Browser(false));
        }
        self.menu = None;

        // The response plot's probe marker (WO-012 increment 5, D3): a drag inside the plot
        // well grabs the marker and moves it horizontally. Read-only — no param, no history,
        // no command-ring traffic (a marker that wrote cutoff would be a second door to the
        // same param, the class of defect this project refuses). The well sits ABOVE the rows,
        // so this branch runs before the slider's; the panel-over-panel routing inc 4 shipped.
        if let Some((node_id, in_plot)) =
            self.inspector.as_ref().map(|il| (il.node, il.plot.is_some_and(|r| r.contains(pos))))
        {
            if in_plot {
                let has_curve = graph
                    .node(node_id)
                    .is_some_and(|n| crate::canvas::inset::declares_curve(&n.spec.module_id));
                if !has_curve {
                    ev.push(CanvasEvent::Refused(
                        "that module declares no response curve — the well says so in words"
                            .to_string(),
                    ));
                    return ev;
                }
                if let Some(hz) = self.marker_freq_at(node_id, pos.x) {
                    self.set_response_marker(node_id, hz);
                    self.interaction = Interaction::Marker { node: node_id, cursor_screen: pos };
                    ev.push(CanvasEvent::Note(format!(
                        "probing the response curve — the marker reads, it never writes ({})",
                        crate::canvas::response::format_hz(hz)
                    )));
                } else {
                    ev.push(CanvasEvent::Refused(
                        "the response plot has no live axes — nothing to probe".to_string(),
                    ));
                }
                return ev;
            }
        }

        // An inspector slider: the drag edits the parameter continuously (tap-to-set already
        // happened via Activate when the recogniser decided it was a tap, not a drag). A BINARY
        // row is a toggle: the press flips it ONCE and there is nothing to drag (operator
        // ruling 2026-10-01).
        if let Some((node_id, step)) = self.inspector.as_ref().and_then(|il| {
            il.row_at(pos).and_then(|i| {
                il.rows[i].steps.iter().position(|c| c.contains(pos)).map(|k| (il.node, k))
            })
        }) {
            self.selection.clear();
            self.selection.nodes.insert(node_id);
            ev.extend(self.flip_step(graph, node_id, step));
            return ev;
        }
        if let Some((node_id, index, editable, toggle, track)) =
            self.inspector.as_ref().and_then(|il| {
                il.row_at(pos).map(|i| {
                    (
                        il.node,
                        il.rows[i].index,
                        il.rows[i].editable,
                        il.rows[i].toggle,
                        il.rows[i].track,
                    )
                })
            })
        {
            if !editable {
                ev.push(CanvasEvent::Refused(
                    "that parameter is not editable in v0 — see the inspector row".to_string(),
                ));
                return ev;
            }
            if toggle {
                if let Some(n) = graph.node(node_id) {
                    if let Some(d) = n.spec.params.get(index) {
                        let cur = n.param_value(index).unwrap_or(0.0);
                        let v = flipped(d, cur);
                        ev.extend(self.apply_param(graph, node_id, index, v, false));
                    }
                }
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
            Hit::Step(node, step) => {
                // A step button pressed: flips once, no continuous drag (the toggle rule's
                // sibling — a button is a decision, not a distance).
                self.selection.clear();
                self.selection.nodes.insert(node);
                ev.extend(self.flip_step(graph, node, step));
                return ev;
            },
            Hit::Key(node, key) => {
                // A keyboard key pressed: flips once, no continuous drag (the step button's
                // rule — a key is a decision, not a distance; operator round 4, D11).
                self.selection.clear();
                self.selection.nodes.insert(node);
                ev.extend(self.flip_key(graph, node, key));
                return ev;
            },
            Hit::WireTrim(id) => {
                // The cable node grabbed (operator round 4, D15): the drag edits the wire's
                // trim — vertical amp, horizontal offset, the offset pinned 0 on audio wires
                // because DC never enters the audio path. The wire itself stays put: this is
                // not a re-patch, and the note names the axes so the drag is no mystery.
                let Some(w) = graph.wire(id) else {
                    ev.push(CanvasEvent::Refused("no such wire".to_string()));
                    return ev;
                };
                let audio =
                    layout.wires.iter().any(|wl| wl.id == id && wl.class == SignalClass::Audio);
                self.selection.clear();
                self.selection.wires.insert(id);
                self.interaction =
                    Interaction::Trim { wire: id, orig: w.trim, acc_screen: Vec2::ZERO, audio };
                ev.push(CanvasEvent::Selection(1));
                ev.push(CanvasEvent::Note(if audio {
                    "dragging the cable node — up/down = amp (an audio cable takes no offset: \
                     DC stays out of the audio path)"
                        .to_string()
                } else {
                    "dragging the cable node — up/down = amp, left/right = offset".to_string()
                }));
            },
            Hit::Param(node, index, track) => {
                // The card's slider, dragged: set at the grab x (no jump-on-move), then ride the
                // shared Param interaction — one undo step per gesture, coalesced updates, the
                // live ring traffic identical to the inspector's drag. A BINARY row flips once
                // and drags no further (the toggle rule).
                if let Some(n) = graph.node(node) {
                    if let Some(d) = n.spec.params.get(index) {
                        if inspector::is_binary(d) {
                            let cur = n.param_value(index).unwrap_or(0.0);
                            let v = flipped(d, cur);
                            ev.extend(self.apply_param(graph, node, index, v, false));
                            return ev;
                        }
                        let v = inspector::value_from_x(d, track, pos.x);
                        ev.extend(self.apply_param(graph, node, index, v, false));
                    }
                }
                self.interaction =
                    Interaction::Param { node, index, cursor_screen: pos, pushed: true };
                return ev;
            },
            Hit::Port(pref, dir) => {
                self.interaction = Interaction::Wire(PendingWire {
                    from: pref,
                    from_dir: dir,
                    cursor_screen: pos,
                    hovered: Some(pref),
                    hovered_param: None,
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
                let src_is_cv = p.from_dir == Direction::Out
                    && graph
                        .port(p.from)
                        .is_some_and(|pt| pt.port_type == sparq_module_api::port::PortType::Cv);
                p.hovered_param =
                    if src_is_cv { layout::param_sink_at(layout, p.cursor_screen) } else { None };
                p.hovered = if p.hovered_param.is_some() {
                    None
                } else {
                    match layout::hit_test(layout, p.cursor_screen, lod) {
                        Hit::Port(pref, _) => Some(pref),
                        _ => None,
                    }
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
                // Geometry comes from this frame's inspector WHEN the drag lives there; a drag
                // that started on a node card reads its track from this frame's canvas layout
                // (increment 6). A stale layout (selection changed mid-drag) simply stops
                // editing rather than guessing.
                let track = self
                    .inspector
                    .as_ref()
                    .and_then(|il| {
                        if il.node != node {
                            return None;
                        }
                        il.rows.iter().find(|r| r.index == index).map(|r| r.track)
                    })
                    .or_else(|| {
                        layout.nodes.iter().find(|n| n.id == node).and_then(|n| {
                            n.param_rows.iter().find(|r| r.index == index).map(|r| r.track)
                        })
                    });
                let desc = graph.node(node).and_then(|n| n.spec.params.get(index).cloned());
                if let (Some(track), Some(d)) = (track, desc) {
                    let v = inspector::value_from_x(&d, track, x);
                    self.apply_param(graph, node, index, v, pushed);
                }
            },
            Interaction::Trim { wire, orig, acc_screen, audio } => {
                // The cable node's own scale (D15): 100 px per unit on both axes — up is
                // louder, right is more positive offset. The raw delta accumulates (this
                // gesture has no fine-scale; its axes ARE the scaling), and every update
                // coalesces into the drag's one history entry.
                acc_screen.x += delta.x;
                acc_screen.y += delta.y;
                let (wire, orig, audio, acc) = (*wire, *orig, *audio, *acc_screen);
                if let Some(o) = orig {
                    let amp = (o.amp - acc.y / 100.0).clamp(0.0, 2.0);
                    let offset = if audio {
                        0.0 // DC never enters the audio path (D15): the horizontal drag is inert
                    } else {
                        (o.offset + acc.x / 100.0).clamp(-1.0, 1.0)
                    };
                    self.apply_trim(graph, wire, Some(WireTrim { amp, offset }), true);
                }
            },
            Interaction::Marker { node, cursor_screen } => {
                *cursor_screen = Vec2::new(cursor_screen.x + delta.x, cursor_screen.y + delta.y);
                let (node, x) = (*node, cursor_screen.x);
                // The mapping reads THIS frame's axes and plot rect; a stale layout (selection
                // changed mid-drag) simply stops moving the marker rather than guessing — the
                // Param arm's discipline, on a reading instead of an edit.
                if let Some(hz) = self.marker_freq_at(node, x) {
                    self.set_response_marker(node, hz);
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
            Interaction::Trim { wire, orig, .. } => self.finish_trim(graph, wire, orig, cancelled),
            Interaction::Marker { node, .. } => self.finish_marker(node, cancelled),
            Interaction::Marquee { start_screen, acc_screen } => {
                self.finish_marquee(graph, start_screen, acc_screen, view)
            },
            Interaction::Idle => Vec::new(),
        }
    }

    /// The marker probe lifted: one log line with the final frequency (mid-drag updates were
    /// silent — the marker moved visibly, and the readout rode with it). NOTHING is committed
    /// because nothing was edited: no history entry, no live-sync note, no command-ring
    /// traffic — a cancelled probe and a finished one leave the same patch behind (D3).
    fn finish_marker(&self, node: NodeId, cancelled: bool) -> Vec<CanvasEvent> {
        let Some(hz) = self.response_marker(node) else { return Vec::new() };
        vec![CanvasEvent::Note(format!(
            "response marker {} at {}",
            if cancelled { "released (cancelled)" } else { "rests" },
            crate::canvas::response::format_hz(hz)
        ))]
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

    /// The cable-node drag committed: one Applied sentence with the final trim (mid-drag
    /// updates were silent and coalesced — the history already holds exactly one entry for
    /// the gesture; `finish_param`'s shape). A CANCEL restores the trim the gesture found
    /// through the same door: the drag was a question, and "no" leaves the wire as it was —
    /// stated, never silent. When the drag never moved the value the restore records
    /// nothing (an undo step that undoes nothing is a lie), and only the sentence speaks.
    fn finish_trim(
        &mut self,
        graph: &mut Graph,
        wire: WireId,
        orig: Option<WireTrim>,
        cancelled: bool,
    ) -> Vec<CanvasEvent> {
        if cancelled {
            let mut ev = self.apply_trim(graph, wire, orig, false);
            ev.push(CanvasEvent::Note(
                "trim drag cancelled — the wire keeps the trim it had".to_string(),
            ));
            return ev;
        }
        match graph.wire(wire).and_then(|w| w.trim) {
            Some(t) => vec![CanvasEvent::Applied(format!(
                "wire trim: amp {:.2} · offset {:.2}",
                t.amp, t.offset
            ))],
            None => Vec::new(),
        }
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
                self.commit(Op::Batch(vec![rm, op]));
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
        self.commit(op);
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
        // Control connection (operator ruling 2026-10-01 r3): dropping the drag on a
        // parameter's sink dot modulates that float setting from this cv source.
        if let Some((node, idx)) = layout::param_sink_at(layout, pos) {
            let Direction::Out = p.from_dir else {
                return vec![CanvasEvent::Refused(
                    "a control wire starts at a cv OUTPUT — drag from the source to the setting"
                        .to_string(),
                )];
            };
            return self.connect_param(graph, p.from, node, idx);
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
                self.commit(op);
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

/// The OTHER pole of a binary parameter (operator ruling 2026-10-01: binary settings are
/// toggle buttons — a tap flips, it never maps x). The midpoint test matches the model's own
/// bool snap ([`crate::canvas::model::Graph::op_set_param`]), so the flip and the clamp agree.
/// The sequencer's pattern-mask bit for `step` (0..16) as a float mask delta — the one place
/// the bit arithmetic lives, so the card, the inspector and the tests cannot drift apart.
fn step_mask_bit(step: usize) -> f32 {
    (1u32 << step.min(15)) as f32
}

/// The quantizer keyboard's pitch-class `key` (0..12) as a float mask delta — the
/// `step_mask_bit` sibling for the CUSTOM scale's mask (operator round 4, D11): one place
/// for the bit arithmetic, so the card's keys, the painter's mask display and the tests
/// cannot drift apart.
fn key_mask_bit(key: usize) -> f32 {
    (1u32 << key.min(11)) as f32
}

fn flipped(desc: &crate::canvas::model::ParamDesc, current: f32) -> f32 {
    let lo = desc.min as f32;
    let hi = desc.max as f32;
    if current >= (lo + hi) / 2.0 {
        lo
    } else {
        hi
    }
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
    use crate::canvas::model::{NodeFlags, NodeSpec, ParamDesc, ParamKind};
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
        // tap the DUPLICATE row — found by ACTION, not by position (increment 5 put RENAME
        // above it; a test that hard-codes a row index is a test that breaks on a menu edit)
        let origin = s.menu_origin(v).unwrap();
        let dup = s
            .menu
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .position(|r| r.action == MenuAction::Duplicate)
            .unwrap();
        let row = s.menu.as_ref().unwrap().row_rect(origin, dup);
        s.on_intent(&mut g, GestureIntent::Activate { pos: row.center() }, &layout, v, &ctx());
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
        let row = il.rows[0].clone();

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

    // ------------------------------------------------- rename entry + inspector scroll (inc 5)

    /// Open the rename sheet the way the menu does: Context on the node, tap the RENAME row.
    fn open_rename_via_menu(g: &mut Graph, s: &mut CanvasState, id: NodeId) {
        let v = view();
        let layout = compute(g, &s.camera, v);
        let body = layout.nodes.iter().find(|n| n.id == id).unwrap().screen.center();
        s.on_intent(g, GestureIntent::Context { pos: body }, &layout, v, &ctx());
        let origin = s.menu_origin(v).unwrap();
        let row = s
            .menu
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .position(|r| r.action == MenuAction::Rename)
            .expect("the node menu offers RENAME");
        let tap = s.menu.as_ref().unwrap().row_rect(origin, row).center();
        s.on_intent(g, GestureIntent::Activate { pos: tap }, &layout, v, &ctx());
    }

    #[test]
    fn the_node_menu_offers_rename_and_the_sheet_opens_pre_filled() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        open_rename_via_menu(&mut g, &mut s, aid);
        assert!(s.menu.is_none(), "the menu closed — the sheet is the deeper modal");
        let r = s.rename().expect("the rename sheet is open");
        assert_eq!(r.node, aid);
        assert_eq!(r.entry.text(), "Sine", "pre-filled with the current title");
        assert_eq!(r.header_text(), "RENAME sparq/syn/sine");
    }

    #[test]
    fn rename_commit_sets_the_title_and_one_undo_restores_it() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        open_rename_via_menu(&mut g, &mut s, aid);
        s.rename_insert(" Kick");
        let ev = s.rename_commit(&mut g);
        assert_eq!(g.node(aid).unwrap().title(), "Sine Kick");
        assert!(
            ev.iter().any(|e| e.message().contains("renamed to `Sine Kick`")),
            "the commit is stated in words: {ev:?}"
        );
        assert!(s.rename().is_none(), "the sheet closed");
        let layout = compute(&g, &s.camera, view());
        s.on_intent(&mut g, GestureIntent::Undo, &layout, view(), &ctx());
        assert_eq!(g.node(aid).unwrap().title(), "Sine", "one undo restores the old name");
    }

    #[test]
    fn an_empty_commit_clears_the_rename_and_an_unchanged_one_is_a_stated_no_op() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        // Set a custom name first.
        open_rename_via_menu(&mut g, &mut s, aid);
        s.rename_set_text("Sub");
        s.rename_commit(&mut g);
        assert_eq!(g.node(aid).unwrap().title(), "Sub");
        // An empty buffer commits None: the module default is back, and it is undoable.
        s.rename_open(&g, aid, Vec2::new(100.0, 100.0));
        s.rename_set_text("   ");
        let ev = s.rename_commit(&mut g);
        assert_eq!(g.node(aid).unwrap().title(), "Sine", "whitespace commits as clear");
        assert!(
            ev.iter().any(|e| e.message().contains("module default")),
            "the clear is stated in words: {ev:?}"
        );
        // Committing the SAME name is a note, not a history entry (an undo step that undoes
        // nothing is a lie).
        let history_before = s.history.can_undo();
        s.rename_open(&g, aid, Vec2::new(100.0, 100.0));
        let ev = s.rename_commit(&mut g);
        assert!(ev.iter().any(|e| e.message().contains("unchanged")), "{ev:?}");
        assert_eq!(s.history.can_undo(), history_before, "no history entry for a no-op");
    }

    #[test]
    fn the_sheet_captures_inside_taps_and_an_outside_tap_cancels_in_words() {
        let (mut g, aid, _) = two_nodes();
        let mut s = CanvasState::new();
        open_rename_via_menu(&mut g, &mut s, aid);
        s.rename_insert("X");
        let v = view();
        let layout = compute(&g, &s.camera, v);
        // A tap INSIDE the sheet: answered in words (the keyboard owns the buffer), stays open.
        let (max_w, _) = crate::canvas::browser::caps(v);
        let origin = s.rename().unwrap().sheet_origin(v, max_w);
        let inside = Vec2::new(origin.x + 10.0, origin.y + 10.0);
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: inside }, &layout, v, &ctx());
        assert!(s.rename().is_some(), "an inside tap does not dismiss the sheet");
        assert!(ev.iter().any(|e| e.message().contains("keyboard")), "{ev:?}");
        // A tap OUTSIDE cancels — stated, name untouched.
        let ev = s.on_intent(
            &mut g,
            GestureIntent::Activate { pos: Vec2::new(v.max.x - 5.0, v.max.y - 5.0) },
            &layout,
            v,
            &ctx(),
        );
        assert!(s.rename().is_none());
        assert_eq!(g.node(aid).unwrap().title(), "Sine", "the buffer was dropped, not committed");
        assert!(ev.iter().any(|e| e.message().contains("cancelled")), "{ev:?}");
        // A louder gesture cancels too: a drag that starts anywhere.
        open_rename_via_menu(&mut g, &mut s, aid);
        s.on_intent(
            &mut g,
            GestureIntent::DragStart { pos: Vec2::new(700.0, 600.0) },
            &layout,
            v,
            &ctx(),
        );
        assert!(s.rename().is_none(), "a drag dismisses the sheet like the menu and browser");
    }

    fn many_param_node(g: &mut Graph) -> NodeId {
        let params: Vec<ParamDesc> = (0..20)
            .map(|i| ParamDesc {
                id: format!("p{i}"),
                name: format!("p{i}"),
                kind: ParamKind::Float,
                unit: None,
                min: 0.0,
                max: 1.0,
                default: 0.5,
            })
            .collect();
        let spec = NodeSpec::new("sparq/util/mixer", "Mixer", vec![]).with_params(params);
        let op = g.op_add_node(spec, Vec2::new(0.0, 300.0));
        nid(&op)
    }

    /// A panel short enough that 20 rows hang below it (the mixer shape the park note named).
    fn short_panel() -> Rect {
        Rect::from_min_size(Vec2::new(900.0, 100.0), Vec2::new(280.0, 400.0))
    }

    #[test]
    fn a_pan_over_the_inspector_scrolls_the_panel_and_over_the_canvas_pans_the_camera() {
        let mut g = Graph::new();
        let id = many_param_node(&mut g);
        let mut s = CanvasState::new();
        let panel = short_panel();
        let node = g.node(id).unwrap().clone();
        let il = inspector::compute_at(&node, panel, s.inspector_scroll(id));
        assert!(il.max_scroll > 0.0, "the test panel must overflow");
        s.set_inspector(Some(il));

        let v = view();
        let layout = compute(&g, &s.camera, v);
        let cam_before = s.camera.origin;
        let in_panel = Vec2::new(panel.min.x + 40.0, panel.min.y + 150.0);
        // Content follows the fingers: a 100 px upward pan reveals rows 100 px further down.
        s.on_intent(
            &mut g,
            GestureIntent::Pan { delta: Vec2::new(0.0, -100.0), center: in_panel },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(s.camera.origin, cam_before, "the camera did NOT move — the panel owns the pan");
        assert!((s.inspector_scroll(id) - 100.0).abs() < 1e-3, "the panel scrolled");
        // The same gesture over the canvas pans the camera and leaves the offset alone.
        let scroll_before = s.inspector_scroll(id);
        s.on_intent(
            &mut g,
            GestureIntent::Pan { delta: Vec2::new(30.0, 0.0), center: Vec2::new(200.0, 200.0) },
            &layout,
            v,
            &ctx(),
        );
        assert!(s.camera.origin.x < cam_before.x, "the camera panned");
        assert_eq!(s.inspector_scroll(id), scroll_before, "the panel did not move");
    }

    #[test]
    fn the_scroll_clamps_and_speaks_only_at_the_ends() {
        let mut g = Graph::new();
        let id = many_param_node(&mut g);
        let mut s = CanvasState::new();
        let panel = short_panel();
        let node = g.node(id).unwrap().clone();
        let il = inspector::compute_at(&node, panel, 0.0);
        let max = il.max_scroll;
        s.set_inspector(Some(il));
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let in_panel = Vec2::new(panel.min.x + 40.0, panel.min.y + 150.0);
        let pan = |s: &mut CanvasState, g: &mut Graph, dy: f32| {
            s.on_intent(
                g,
                GestureIntent::Pan { delta: Vec2::new(0.0, dy), center: in_panel },
                &layout,
                v,
                &ctx(),
            )
        };
        // Mid-scroll: silent (the moving rows are the feedback — the camera-pan precedent).
        assert!(pan(&mut s, &mut g, -50.0).is_empty());
        // Past the end: clamped, and the boundary IS stated (further panning does nothing).
        let ev = pan(&mut s, &mut g, -(max + 200.0));
        assert_eq!(s.inspector_scroll(id), max, "clamped at the end");
        assert!(ev.iter().any(|e| e.message().contains("last row")), "{ev:?}");
        assert!(pan(&mut s, &mut g, -50.0).is_empty(), "already at the end: no repeat narration");
        // Back past the top: clamped at zero and stated.
        let ev = pan(&mut s, &mut g, max + 200.0);
        assert_eq!(s.inspector_scroll(id), 0.0);
        assert!(ev.iter().any(|e| e.message().contains("first row")), "{ev:?}");
    }

    #[test]
    fn a_panel_that_fits_has_nothing_to_scroll_and_a_new_selection_starts_at_the_top() {
        let mut g = Graph::new();
        let op = g.op_add_node(gain(), Vec2::ZERO); // two params: fits any real panel
        let small_id = nid(&op);
        let mut s = CanvasState::new();
        let node = g.node(small_id).unwrap().clone();
        let il = inspector::compute(&node, view());
        assert_eq!(il.max_scroll, 0.0);
        s.set_inspector(Some(il));
        // A pan over a fitting panel: captured (no camera surprise behind the panel), silent —
        // the panel visibly shows every row and no thumb, which is its own explanation.
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let cam = s.camera.origin;
        let ev = s.on_intent(
            &mut g,
            GestureIntent::Pan { delta: Vec2::new(0.0, -40.0), center: Vec2::new(100.0, 100.0) },
            &layout,
            v,
            &ctx(),
        );
        assert!(ev.is_empty());
        assert_eq!(s.camera.origin, cam, "the panel captured the gesture even with nothing to do");

        // Scroll a LONG panel, then inspect a DIFFERENT node: the stored offset must not follow
        // the selection — the shell asks `inspector_scroll(new_node)` and gets zero.
        let mut g2 = Graph::new();
        let big = many_param_node(&mut g2);
        let big_node = g2.node(big).unwrap().clone();
        let il = inspector::compute_at(&big_node, short_panel(), 0.0);
        s.set_inspector(Some(il));
        s.scroll_inspector(-120.0);
        assert!((s.inspector_scroll(big) - 120.0).abs() < 1e-3, "the scroll sticks for its node");
        assert_eq!(s.inspector_scroll(big + 77), 0.0, "a different node starts at the top");
        // Closing the panel drops the offset entirely.
        s.set_inspector(None);
        assert_eq!(s.inspector_scroll(big), 0.0);
    }

    #[test]
    fn the_patch_ledger_classifies_what_a_live_engine_can_hear() {
        // WO-012 increment 2, the one op→sync door: structural edits owe a rebuild + re-stage,
        // param edits owe a set_params snapshot, and edits the engine cannot hear (a move, a
        // rename) mark NOTHING — a sync that rebuilds for a pixel move would reset module state
        // for no reason an operator could name.
        let mut g = Graph::new();
        let id = nid(&g.op_add_node(sine_with_params(), Vec2::ZERO));
        let g2 = nid(&g.op_add_node(gain(), Vec2::new(300.0, 0.0)));
        let mut s = CanvasState::new();

        // A param edit: param-dirty, NOT structural — and deduped per node.
        s.apply_param(&mut g, id, 0, 500.0, false);
        s.apply_param(&mut g, id, 0, 600.0, false);
        let c = s.take_patch_changes();
        assert!(!c.structural, "a param edit is not structural");
        assert_eq!(c.param_nodes, vec![id], "one entry per node, not per edit");
        assert!(s.take_patch_changes().is_empty(), "the take left an empty ledger");

        // A coalesced drag frame AFTER a take re-marks (the merge replaces the top entry
        // without pushing — the ledger must not depend on the push).
        s.apply_param(&mut g, id, 0, 700.0, false);
        s.take_patch_changes();
        s.apply_param(&mut g, id, 0, 800.0, true);
        assert_eq!(s.take_patch_changes().param_nodes, vec![id], "the merge re-marked");

        // Ops the engine cannot hear.
        s.commit(Op::MoveNode { id, from: Vec2::ZERO, to: Vec2::new(10.0, 10.0) });
        s.commit(g.op_rename(id, Some("voice".into())).unwrap());
        assert!(s.take_patch_changes().is_empty(), "a move and a rename are inaudible");

        // Structural edits, each on its own.
        s.commit(g.op_add_wire(PortRef::new(id, 0), PortRef::new(g2, 0)));
        assert!(s.take_patch_changes().structural, "a wire is structural");
        let flags = NodeFlags { bypassed: true, ..NodeFlags::default() };
        s.commit(g.op_set_flags(id, flags).unwrap());
        assert!(s.take_patch_changes().structural, "a flag is structural");

        // Undo goes through the same door: undoing the flag edit is structural again…
        s.undo(&mut g);
        assert!(s.take_patch_changes().structural, "undo of a flag edit re-syncs");
        // …and redo likewise.
        s.redo(&mut g);
        assert!(s.take_patch_changes().structural, "redo of a flag edit re-syncs");

        // SET MASTER is not an Op, but it is structural: which node feeds the listener moved.
        s.run_menu_action(&mut g, MenuTarget::Node(g2), MenuAction::SetMaster, Vec2::ZERO, view());
        assert_eq!(s.master, Some(g2));
        assert!(s.take_patch_changes().structural, "a master handover is structural");
    }

    // ------------------------------------------------- the response-plot marker (increment 5)

    /// An `flt/svf` node with the manifest's own param order (0 cutoff · 1 resonance · 2 mode ·
    /// 3 mod depth) — pinned against the registry manifest by a sparq-app test.
    fn svf_node(g: &mut Graph) -> NodeId {
        let params = vec![
            ParamDesc {
                id: "cutoff".into(),
                name: "Cutoff".into(),
                kind: ParamKind::Float,
                unit: Some("Hz".into()),
                min: 10.0,
                max: 20_000.0,
                default: 1_000.0,
            },
            ParamDesc {
                id: "resonance".into(),
                name: "Resonance".into(),
                kind: ParamKind::Float,
                unit: None,
                min: 0.0,
                max: 1.0,
                default: 0.2,
            },
            ParamDesc {
                id: "mode".into(),
                name: "Mode".into(),
                kind: ParamKind::Int,
                unit: None,
                min: 0.0,
                max: 4.0,
                default: 0.0,
            },
        ];
        let spec = NodeSpec::new(
            crate::canvas::inset::SVF_ID,
            "SVF",
            vec![
                audio("in", Direction::In, ChannelSet::Stereo),
                audio("out", Direction::Out, ChannelSet::Stereo),
            ],
        )
        .with_params(params);
        nid(&g.op_add_node(spec, Vec2::ZERO))
    }

    /// The inspected-svf scaffolding the marker tests share: graph, state with the inspector
    /// and the live axes stored, and the plot rect to aim at.
    fn svf_inspected() -> (Graph, CanvasState, NodeId, Rect) {
        let mut g = Graph::new();
        let id = svf_node(&mut g);
        let mut s = CanvasState::new();
        s.selection.nodes.insert(id);
        let panel = Rect::from_min_size(Vec2::new(900.0, 100.0), Vec2::new(400.0, 600.0));
        let node = g.node(id).unwrap().clone();
        let il = inspector::compute_at(&node, panel, 0.0);
        let plot = il.plot.expect("svf declares a curve, so the well is reserved");
        s.set_inspector(Some(il));
        s.set_response_axes(Some((id, crate::canvas::response::Axes::at(48_000))));
        (g, s, id, plot)
    }

    #[test]
    fn the_marker_probe_reads_the_curve_and_moves_no_audio() {
        let (mut g, mut s, id, plot) = svf_inspected();
        let cutoff_before = g.node(id).unwrap().param_value(0);

        // Drag from the plot's centre to the right: the marker follows the finger in log-f.
        let start = plot.center();
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragStart { pos: start },
            &CanvasLayout::default(),
            view(),
            &ctx(),
        );
        assert!(matches!(s.interaction, Interaction::Marker { .. }), "{:?}", s.interaction);
        assert!(
            ev.iter()
                .any(|e| matches!(e, CanvasEvent::Note(t) if t.contains("reads, it never writes"))),
            "the probe says what it is: {ev:?}"
        );
        let at_start = s.response_marker(id).unwrap();
        let want_start = crate::canvas::response::Axes::at(48_000).freq_of_x(plot, start.x);
        assert!((at_start - want_start).abs() < 1e-6, "the grab maps through the plot's own axes");

        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate { delta: Vec2::new(plot.width() / 4.0, 0.0), scale: 1.0 },
            &CanvasLayout::default(),
            view(),
            &ctx(),
        );
        let moved = s.response_marker(id).unwrap();
        assert!(
            moved > at_start * 2.0,
            "a quarter-plot right is a big log-f move: {at_start} → {moved}"
        );

        let end_pos = Vec2::new(start.x + plot.width() / 4.0, start.y);
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: end_pos, cancelled: false },
            &CanvasLayout::default(),
            view(),
            &ctx(),
        );
        assert!(matches!(s.interaction, Interaction::Idle), "the drag is over");
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Note(t) if t.contains("response marker"))),
            "the lift is stated in words: {ev:?}"
        );

        // The proof burden (D3): the marker moved NO audio. The patch is untouched, the history
        // is empty, and the live-sync ledger heard nothing — no re-stage, no command-ring traffic.
        assert_eq!(g.node(id).unwrap().param_value(0), cutoff_before, "cutoff stays the slider's");
        assert_eq!(s.history.undo_len(), 0, "a probe is not an edit — nothing to undo");
        assert!(s.take_patch_changes().is_empty(), "the engine hears nothing about a probe");
        assert!(
            !ev.iter().any(|e| matches!(e, CanvasEvent::Applied(_))),
            "nothing was APPLIED — a reading never claims an edit"
        );
    }

    #[test]
    fn tap_to_place_sets_the_marker_and_the_stored_value_clamps() {
        let (mut g, mut s, id, plot) = svf_inspected();
        let axes = crate::canvas::response::Axes::at(48_000);
        let tap = Vec2::new(plot.min.x + plot.width() * 0.75, plot.center().y);
        let ev = s.on_intent(
            &mut g,
            GestureIntent::Activate { pos: tap },
            &CanvasLayout::default(),
            view(),
            &ctx(),
        );
        let hz = s.response_marker(id).expect("the tap placed the marker");
        assert!((hz - axes.freq_of_x(plot, tap.x)).abs() < 1e-6, "placed at the tapped frequency");
        assert!(
            ev.iter().any(
                |e| matches!(e, CanvasEvent::Note(t) if t.contains("cutoff stays the slider"))
            ),
            "{ev:?}"
        );
        assert_eq!(s.history.undo_len(), 0, "tap-to-place is display state too");

        // The setter's clamp: the marker cannot leave the declared band, and a NaN reads the floor.
        s.set_response_marker(id, 1.0e9);
        assert_eq!(s.response_marker(id), Some(crate::canvas::response::FREQ_MAX_HZ));
        s.set_response_marker(id, -5.0);
        assert_eq!(s.response_marker(id), Some(crate::canvas::response::FREQ_MIN_HZ));
        s.set_response_marker(id, f64::NAN);
        assert_eq!(s.response_marker(id), Some(crate::canvas::response::FREQ_MIN_HZ));
    }

    #[test]
    fn the_marker_resets_on_selection_change_and_dies_with_the_panel() {
        let (mut g, mut s, id, plot) = svf_inspected();
        s.set_response_marker(id, 1_234.0);
        assert_eq!(s.response_marker(id), Some(1_234.0));
        // A different node inspected (the shell stores the fresh layout every frame): the
        // marker is the OLD node's display state and does not survive.
        let other = nid(&g.op_add_node(sine(), Vec2::new(0.0, 300.0)));
        let panel = Rect::from_min_size(Vec2::new(900.0, 100.0), Vec2::new(400.0, 600.0));
        let il = inspector::compute_at(&g.node(other).unwrap().clone(), panel, 0.0);
        s.set_inspector(Some(il));
        assert_eq!(s.response_marker(id), None, "a selection change resets the probe");
        // The same node re-inspected also starts unplaced (set_inspector saw the change).
        s.set_response_marker(other, 900.0);
        s.set_inspector(None);
        assert_eq!(s.response_marker(other), None, "closing the panel clears the probe");
        let _ = plot;
    }

    #[test]
    fn a_no_curve_module_has_no_plot_to_probe_and_says_nothing_about_curves() {
        // Operator ruling 2026-09-30: the no-curve description box is GONE — so there is no
        // well rect at all, and a drag or tap where it used to be is plain panel space: no
        // marker, no refusal sentence about curves, nothing grabbed.
        let mut g = Graph::new();
        let id = nid(&g.op_add_node(sine(), Vec2::ZERO));
        let mut s = CanvasState::new();
        let panel = Rect::from_min_size(Vec2::new(900.0, 100.0), Vec2::new(400.0, 600.0));
        let il = inspector::compute_at(&g.node(id).unwrap().clone(), panel, 0.0);
        assert_eq!(il.plot, None, "no curve declared: no well reserved");
        let where_the_well_was = Vec2::new(panel.center().x, il.title.max.y + 60.0);
        s.set_inspector(Some(il)); // no axes stored: sine declares no curve

        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragStart { pos: where_the_well_was },
            &CanvasLayout::default(),
            view(),
            &ctx(),
        );
        assert_eq!(s.response_marker(id), None, "nothing to grab, nothing placed");
        assert!(
            !ev.iter().any(|e| e.message().contains("response curve")),
            "the panel no longer speaks about curves at all: {ev:?}"
        );
        s.on_intent(
            &mut g,
            GestureIntent::Activate { pos: where_the_well_was },
            &CanvasLayout::default(),
            view(),
            &ctx(),
        );
        assert_eq!(s.response_marker(id), None, "the tap placed nothing");
    }

    #[test]
    fn a_marker_drag_with_stale_axes_stops_instead_of_guessing() {
        let (mut g, mut s, id, plot) = svf_inspected();
        s.on_intent(
            &mut g,
            GestureIntent::DragStart { pos: plot.center() },
            &CanvasLayout::default(),
            view(),
            &ctx(),
        );
        let at_start = s.response_marker(id).unwrap();
        // The shell stops supplying axes mid-drag (selection changed elsewhere): the marker
        // holds its last honest value — the Param arm's stale-layout discipline.
        s.set_response_axes(None);
        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate { delta: Vec2::new(60.0, 0.0), scale: 1.0 },
            &CanvasLayout::default(),
            view(),
            &ctx(),
        );
        assert_eq!(s.response_marker(id), Some(at_start), "no axes, no move — never a guess");
    }

    // ---------------------------------------------------- DELETE key + the permanent Main Out

    fn wire_id(op: &Op) -> WireId {
        match op {
            Op::AddWire(w) => w.id,
            _ => unreachable!(),
        }
    }

    fn out_main_spec() -> NodeSpec {
        NodeSpec::new(
            crate::canvas::OUT_MAIN_ID,
            "Main Out",
            vec![
                audio("in", Direction::In, ChannelSet::Stereo),
                audio("out", Direction::Out, ChannelSet::Stereo),
            ],
        )
    }

    #[test]
    fn delete_removes_the_selection_and_an_empty_selection_refuses_in_words() {
        let (mut g, aid, bid) = two_nodes();
        let wid = wire_id(&g.op_add_wire(PortRef::new(aid, 0), PortRef::new(bid, 0)));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        // nothing selected → a refusal with the remedy, never a silence
        let ev = s.on_intent(&mut g, GestureIntent::Delete, &layout, v, &ctx());
        assert!(
            ev.iter()
                .any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("nothing selected"))),
            "{ev:?}"
        );
        assert_eq!(g.node_count(), 2);
        // a selected wire → gone
        s.selection.wires.insert(wid);
        s.on_intent(&mut g, GestureIntent::Delete, &layout, v, &ctx());
        assert!(g.wires().is_empty());
        assert!(s.selection.is_empty(), "the selection clears with its content");
        // a selected node → gone
        s.selection.nodes.insert(aid);
        let ev = s.on_intent(&mut g, GestureIntent::Delete, &layout, v, &ctx());
        assert_eq!(g.node_count(), 1);
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Applied(t) if t.contains("deleted 1"))),
            "{ev:?}"
        );
        // each delete rode the history: two undos restore the wire and the node
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.wires().len(), 1);
    }

    #[test]
    fn the_permanent_main_out_cannot_be_deleted_duplicated_or_spawned() {
        // Operator ruling 2026-10-01: Main Out is not a module the user creates or deletes —
        // every door refuses in words and the node survives all of them.
        let mut g = Graph::new();
        let m = nid(&g.op_add_node(out_main_spec(), Vec2::ZERO));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        // DEL on a selected Main Out
        s.selection.nodes.insert(m);
        let ev = s.on_intent(&mut g, GestureIntent::Delete, &layout, v, &ctx());
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("permanent"))),
            "{ev:?}"
        );
        assert_eq!(g.node_count(), 1);
        // the long-press menu's DELETE
        let ev = s.run_menu_action(&mut g, MenuTarget::Node(m), MenuAction::Delete, Vec2::ZERO, v);
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("permanent"))),
            "{ev:?}"
        );
        // DUPLICATE
        let ev = s.duplicate(&mut g, m);
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("permanent"))),
            "{ev:?}"
        );
        // a spawn through the catalogue door
        s.set_catalog(vec![BrowserItem::new(out_main_spec())]);
        let ev = s.spawn_module(&mut g, crate::canvas::OUT_MAIN_ID, Vec2::new(400.0, 0.0));
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("permanent"))),
            "{ev:?}"
        );
        assert_eq!(g.node_count(), 1, "every door refused; the one Main Out stands");
    }

    #[test]
    fn a_locked_node_survives_a_select_all_delete_sweep() {
        let (mut g, aid, bid) = two_nodes();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let mut flags = g.node(aid).unwrap().flags;
        flags.locked = true;
        let _ = g.op_set_flags(aid, flags);
        s.selection.nodes.insert(aid);
        s.selection.nodes.insert(bid);
        let ev = s.on_intent(&mut g, GestureIntent::Delete, &layout, v, &ctx());
        assert_eq!(g.node_count(), 1, "the unlocked node went");
        assert!(g.node(aid).is_some(), "the locked node stands");
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r) if r.contains("locked"))),
            "and the refusal says why: {ev:?}"
        );
    }

    #[test]
    fn deleting_a_middle_module_keeps_the_chain_connected() {
        // Operator ruling 2026-10-01: deleting a module splices its feed onto its feedees —
        // the chain stays connected, and ONE undo restores the world before the delete.
        let (mut g, aid, bid) = two_nodes();
        let cid = nid(&g.op_add_node(gain(), Vec2::new(800.0, 0.0)));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        s.connect_ports(&mut g, PortRef::new(aid, 0), PortRef::new(bid, 0), &ctx());
        s.connect_ports(&mut g, PortRef::new(bid, 1), PortRef::new(cid, 0), &ctx());
        assert_eq!(g.wires().len(), 2);
        s.selection.nodes.insert(bid);
        let ev = s.on_intent(&mut g, GestureIntent::Delete, &layout, v, &ctx());
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.wires().len(), 1, "the chain is spliced past the deleted node");
        assert_eq!(g.wires()[0].from, PortRef::new(aid, 0));
        assert_eq!(g.wires()[0].to, PortRef::new(cid, 0));
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Note(n) if n.contains("spliced"))),
            "and the shell says so: {ev:?}"
        );
        // one undo restores the node AND both original wires AND removes the splice
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.wires().len(), 2);
    }

    #[test]
    fn a_binary_setting_flips_on_tap_and_never_maps_x() {
        // Operator ruling 2026-10-01: Mute-shaped params are toggle buttons — the tap flips,
        // wherever on the row it lands, and a drag flips exactly once (no continuous edit).
        let mute = crate::canvas::model::ParamDesc {
            id: "mute".into(),
            name: "Mute".into(),
            kind: sparq_module_api::manifest::ParamKind::Int,
            unit: Some("x".into()),
            min: 0.0,
            max: 1.0,
            default: 0.0,
        };
        let spec = NodeSpec::new("sparq/util/mutey", "Mutey", vec![]).with_params(vec![mute]);
        let mut g = Graph::new();
        let id = nid(&g.op_add_node(spec, Vec2::ZERO));
        let mut s = CanvasState::new();
        let panel = Rect::from_min_size(Vec2::new(800.0, 100.0), Vec2::new(400.0, 600.0));
        let il = inspector::compute(g.node(id).unwrap(), panel);
        assert!(il.rows[0].toggle, "the row is flagged a toggle");
        s.set_inspector(Some(il.clone()));
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let row = il.rows[0].clone();
        // tap at the far LEFT of the track — x is irrelevant: OFF flips ON
        let left = Vec2::new(row.track.min.x + 1.0, row.track.center().y);
        s.on_intent(&mut g, GestureIntent::Activate { pos: left }, &layout, v, &ctx());
        assert_eq!(g.node(id).unwrap().param_value(0).unwrap(), 1.0);
        // tap at the far RIGHT — ON flips OFF
        let right = Vec2::new(row.track.max.x - 1.0, row.track.center().y);
        s.on_intent(&mut g, GestureIntent::Activate { pos: right }, &layout, v, &ctx());
        assert_eq!(g.node(id).unwrap().param_value(0).unwrap(), 0.0);
        // a drag flips ONCE and starts no continuous interaction
        let before = s.history.undo_len();
        s.on_intent(&mut g, GestureIntent::DragStart { pos: left }, &layout, v, &ctx());
        assert!(matches!(s.interaction, Interaction::Idle), "{:?}", s.interaction);
        assert_eq!(g.node(id).unwrap().param_value(0).unwrap(), 1.0);
        assert_eq!(s.history.undo_len(), before + 1, "one flip = one undo step");
    }

    // --------------------- operator round 4: cable nodes (D15) + the quantizer keyboard (D11)

    /// A cv wire between two cv nodes — the NON-audio case for the trim drag (its offset is
    /// live; an audio wire's is pinned 0). The lfo's own well lifts its port, so the wire is a
    /// slung bézier — the arc midpoint, not the sample middle, is what the handle sits on.
    fn cv_pair() -> (Graph, NodeId, NodeId) {
        let mut g = Graph::new();
        let src =
            NodeSpec::new("sparq/mod/lfo", "LFO", vec![cv("o", Direction::Out, CvRange::Unipolar)]);
        let dst =
            NodeSpec::new("sparq/x/sink", "Sink", vec![cv("i", Direction::In, CvRange::Unipolar)]);
        let a = g.op_add_node(src, Vec2::ZERO);
        let b = g.op_add_node(dst, Vec2::new(400.0, 0.0));
        (g, nid(&a), nid(&b))
    }

    /// A `util/quant` card the D11 shape: `scale` (0 = Custom, the dropdown's first row) then
    /// `custom-mask` — the twelve-bit mask the keyboard's keys flip. The params are the whole
    /// contract here; the ports only keep the card honest.
    fn quant_node(g: &mut Graph) -> NodeId {
        let params = vec![
            ParamDesc {
                id: "scale".into(),
                name: "Scale".into(),
                kind: ParamKind::Int,
                unit: None,
                min: 0.0,
                max: 14.0,
                default: 0.0,
            },
            ParamDesc {
                id: "custom-mask".into(),
                name: "Custom Scale".into(),
                kind: ParamKind::Int,
                unit: None,
                min: 0.0,
                max: 4095.0,
                default: 0.0,
            },
        ];
        let spec = NodeSpec::new(
            crate::canvas::QUANT_ID,
            "Quant",
            vec![
                cv("pitch", Direction::In, CvRange::Bipolar),
                cv("pitch", Direction::Out, CvRange::Bipolar),
            ],
        )
        .with_params(params);
        nid(&g.op_add_node(spec, Vec2::ZERO))
    }

    #[test]
    fn tapping_the_hover_ghost_inserts_an_identity_node_and_one_undo_removes_it() {
        let (mut g, aid, bid) = two_nodes();
        let wid = wire_id(&g.op_add_wire(PortRef::new(aid, 0), PortRef::new(bid, 0)));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let tp = layout.wires.iter().find(|w| w.id == wid).unwrap().trim_point;
        assert_eq!(g.wire(wid).unwrap().trim, None, "the wire starts clean");
        // A tap on the clean wire's ghost: the node is inserted AT REST — identity — so the
        // handle appears and nothing is heard, and the note teaches the gesture vocabulary.
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tp }, &layout, v, &ctx());
        assert_eq!(g.wire(wid).unwrap().trim, Some(WireTrim::identity()), "inserted at identity");
        assert!(s.selection.wires.contains(&wid), "the insert selects the wire");
        assert!(
            ev.iter().any(
                |e| matches!(e, CanvasEvent::Applied(t) if t == "wire trim: amp 1.00 · offset 0.00")
            ),
            "{ev:?}"
        );
        assert!(
            ev.iter()
                .any(|e| matches!(e, CanvasEvent::Note(n) if n.contains("tap the node to remove"))),
            "the vocabulary is taught on insert: {ev:?}"
        );
        // The ledger: a trim is STRUCTURAL (the door's decided sentence — trims re-stage).
        assert!(s.take_patch_changes().structural, "SetTrim marks structural");
        // ONE undo removes the insert: the wire runs clean again.
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert_eq!(g.wire(wid).unwrap().trim, None, "one undo removes the insert");
        assert!(s.take_patch_changes().structural, "the undo re-stages too");
        // A tap elsewhere on the wire only selects — the ghost's door is its capture, not the
        // whole cable.
        let grab = layout.wires.iter().find(|w| w.id == wid).unwrap().grab_from;
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: grab }, &layout, v, &ctx());
        assert_eq!(g.wire(wid).unwrap().trim, None, "no node away from the ghost");
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Selection(1))), "{ev:?}");
        // Insert again, then tap the HANDLE: the removal the insert note promised.
        s.on_intent(&mut g, GestureIntent::Activate { pos: tp }, &layout, v, &ctx());
        let layout = compute(&g, &s.camera, v);
        assert_eq!(
            layout::hit_test(&layout, tp, s.camera.lod()),
            Hit::WireTrim(wid),
            "the door and the eye agree on where the node is"
        );
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tp }, &layout, v, &ctx());
        assert_eq!(g.wire(wid).unwrap().trim, None, "the tap on the node removes it");
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Applied(t) if t == "wire trim removed")),
            "{ev:?}"
        );
    }

    #[test]
    fn a_trim_drag_is_one_undo_step_keeping_the_original_from() {
        let (mut g, lid, sid) = cv_pair();
        let wid = wire_id(&g.op_add_wire(PortRef::new(lid, 0), PortRef::new(sid, 0)));
        // The wire already carries the trim the finger is about to find (setup, not a
        // gesture — it rides no history, so the drag's entry is unambiguously its own).
        g.op_set_trim(wid, Some(WireTrim { amp: 0.9, offset: 0.1 })).unwrap();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let tp = layout.wires.iter().find(|w| w.id == wid).unwrap().trim_point;
        let undo_before = s.history.undo_len();
        let ev = s.on_intent(&mut g, GestureIntent::DragStart { pos: tp }, &layout, v, &ctx());
        assert!(
            matches!(s.interaction, Interaction::Trim { audio: false, .. }),
            "{:?}",
            s.interaction
        );
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Note(n)
                    if n.contains("up/down = amp") && n.contains("left/right = offset"))),
            "the press names the axes: {ev:?}"
        );
        // Many updates, ONE gesture: 50 px up and 30 px right in five steps → amp 1.4, offset 0.4.
        for _ in 0..5 {
            s.on_intent(
                &mut g,
                GestureIntent::DragUpdate { delta: Vec2::new(6.0, -10.0), scale: 1.0 },
                &layout,
                v,
                &ctx(),
            );
        }
        let t = g.wire(wid).unwrap().trim.unwrap();
        assert!((t.amp - 1.4).abs() < 1e-3, "{t:?}");
        assert!((t.offset - 0.4).abs() < 1e-3, "{t:?}");
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: Vec2::new(tp.x + 30.0, tp.y - 50.0), cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(s.history.undo_len(), undo_before + 1, "one drag = one undo step");
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Applied(t)
                    if t.contains("amp 1.40") && t.contains("offset 0.40"))),
            "the lift states the final trim once: {ev:?}"
        );
        // The coalesced entry kept the ORIGINAL from: one undo returns the trim the finger
        // found, not the drag's first waypoint.
        match s.history.top() {
            Some(Op::SetTrim { from, to, .. }) => {
                assert_eq!(*from, Some(WireTrim { amp: 0.9, offset: 0.1 }), "the original from");
                assert_eq!(*to, Some(t), "and the dragged value");
            },
            other => unreachable!("{other:?}"),
        }
        assert!(s.take_patch_changes().structural, "a trim drag is structural");
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert_eq!(
            g.wire(wid).unwrap().trim,
            Some(WireTrim { amp: 0.9, offset: 0.1 }),
            "undo restores the trim the finger found"
        );
    }

    #[test]
    fn an_audio_cables_offset_stays_pinned_at_zero() {
        let (mut g, aid, bid) = two_nodes(); // sine → gain: an AUDIO wire
        let wid = wire_id(&g.op_add_wire(PortRef::new(aid, 0), PortRef::new(bid, 0)));
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let tp = layout.wires.iter().find(|w| w.id == wid).unwrap().trim_point;
        // Insert through the tap door, then grab the handle.
        s.on_intent(&mut g, GestureIntent::Activate { pos: tp }, &layout, v, &ctx());
        let layout = compute(&g, &s.camera, v);
        let ev = s.on_intent(&mut g, GestureIntent::DragStart { pos: tp }, &layout, v, &ctx());
        assert!(
            matches!(s.interaction, Interaction::Trim { audio: true, .. }),
            "{:?}",
            s.interaction
        );
        assert!(
            ev.iter().any(
                |e| matches!(e, CanvasEvent::Note(n) if n.contains("DC stays out of the audio path"))
            ),
            "the press says why the horizontal axis is dead: {ev:?}"
        );
        // A diagonal drag: the amp moves, the offset stays 0 — DC never enters the audio path.
        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate { delta: Vec2::new(80.0, -30.0), scale: 1.0 },
            &layout,
            v,
            &ctx(),
        );
        let t = g.wire(wid).unwrap().trim.unwrap();
        assert!((t.amp - 1.3).abs() < 1e-3, "the vertical axis is live: {t:?}");
        assert_eq!(t.offset, 0.0, "the horizontal axis is inert on audio: {t:?}");
        s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: Vec2::new(tp.x + 80.0, tp.y - 30.0), cancelled: false },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(g.wire(wid).unwrap().trim.unwrap().offset, 0.0, "and stays so at the lift");
    }

    #[test]
    fn a_cancelled_trim_drag_restores_the_trim_the_finger_found() {
        let (mut g, lid, sid) = cv_pair();
        let wid = wire_id(&g.op_add_wire(PortRef::new(lid, 0), PortRef::new(sid, 0)));
        // The wire already carries a trim the finger is about to find (setup, not a gesture —
        // it rides no history).
        g.op_set_trim(wid, Some(WireTrim { amp: 0.7, offset: -0.4 })).unwrap();
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let tp = layout.wires.iter().find(|w| w.id == wid).unwrap().trim_point;
        s.on_intent(&mut g, GestureIntent::DragStart { pos: tp }, &layout, v, &ctx());
        // 60 px left, 45 px down: amp falls, the offset runs INTO its clamp at −1.
        s.on_intent(
            &mut g,
            GestureIntent::DragUpdate { delta: Vec2::new(-60.0, 45.0), scale: 1.0 },
            &layout,
            v,
            &ctx(),
        );
        let moved = g.wire(wid).unwrap().trim.unwrap();
        assert!((moved.amp - 0.25).abs() < 1e-3, "{moved:?}");
        assert!(
            (moved.offset - (-1.0)).abs() < 1e-3,
            "the drag clamps at the model's range: {moved:?}"
        );
        // Cancel: the drag was a question; "no" leaves the wire as it was — stated in words.
        let ev = s.on_intent(
            &mut g,
            GestureIntent::DragEnd { pos: tp, cancelled: true },
            &layout,
            v,
            &ctx(),
        );
        assert_eq!(
            g.wire(wid).unwrap().trim,
            Some(WireTrim { amp: 0.7, offset: -0.4 }),
            "the cancel restores the found trim exactly"
        );
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Note(n) if n.contains("cancelled"))),
            "{ev:?}"
        );
        assert!(matches!(s.interaction, Interaction::Idle), "the drag is over");
    }

    #[test]
    fn a_key_tap_flips_its_mask_bit_through_the_param_door() {
        let mut g = Graph::new();
        let q = quant_node(&mut g);
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let nq = layout.nodes.iter().find(|n| n.id == q).unwrap();
        assert_eq!(nq.key_param, Some(1), "the keys edit `custom-mask`");
        let tap3 = nq.key_cells[3].center();
        assert_eq!(layout::hit_test(&layout, tap3, s.camera.lod()), Hit::Key(q, 3));
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tap3 }, &layout, v, &ctx());
        assert_eq!(g.node(q).unwrap().param_value(1), Some(8.0), "bit 3 → mask 8");
        assert!(s.selection.nodes.contains(&q), "the tap selects the node");
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Applied(_))), "{ev:?}");
        // The live ledger hears it: a key is a PARAM edit, not structural — the mask rides
        // the command ring like any knob.
        let c = s.take_patch_changes();
        assert!(!c.structural, "a key edit is not structural");
        assert_eq!(c.param_nodes, vec![q], "the quant node owes a set_params snapshot");
        // A second tap clears the bit; one undo per flip.
        s.on_intent(&mut g, GestureIntent::Activate { pos: tap3 }, &layout, v, &ctx());
        assert_eq!(g.node(q).unwrap().param_value(1), Some(0.0));
        s.on_intent(&mut g, GestureIntent::Undo, &layout, v, &ctx());
        assert_eq!(g.node(q).unwrap().param_value(1), Some(8.0), "one undo per flip");
        // The twelfth key is the top bit: the mask is twelve bits wide, like the keyboard.
        let tap11 = nq.key_cells[11].center();
        s.on_intent(&mut g, GestureIntent::Activate { pos: tap11 }, &layout, v, &ctx());
        assert_eq!(g.node(q).unwrap().param_value(1), Some(8.0 + 2048.0));
        // The PRESS path flips once and never drags (the step button's rule).
        let before = s.history.undo_len();
        let tap0 = nq.key_cells[0].center();
        s.on_intent(&mut g, GestureIntent::DragStart { pos: tap0 }, &layout, v, &ctx());
        assert!(matches!(s.interaction, Interaction::Idle), "{:?}", s.interaction);
        assert_eq!(s.history.undo_len(), before + 1, "a press is one flip, no drag");
        assert_eq!(g.node(q).unwrap().param_value(1), Some(8.0 + 2048.0 + 1.0));
    }

    #[test]
    fn a_key_tap_on_a_preset_scale_refuses_in_words() {
        let mut g = Graph::new();
        let q = quant_node(&mut g);
        g.op_set_param(q, 0, 5.0).unwrap(); // a preset scale — NOT Custom
        let mut s = CanvasState::new();
        let v = view();
        let layout = compute(&g, &s.camera, v);
        let tap = layout.nodes.iter().find(|n| n.id == q).unwrap().key_cells[2].center();
        let before = s.history.undo_len();
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tap }, &layout, v, &ctx());
        assert!(
            ev.iter().any(|e| matches!(e, CanvasEvent::Refused(r)
                    if r.contains("CUSTOM") && r.contains("scale list"))),
            "the refusal carries the remedy: {ev:?}"
        );
        assert_eq!(g.node(q).unwrap().param_value(1), Some(0.0), "the mask is untouched");
        assert_eq!(s.history.undo_len(), before, "a refusal is not an operation");
        assert!(s.take_patch_changes().is_empty(), "the engine hears nothing about a refusal");
        // Picking Custom (scale → 0) opens the keyboard again — the remedy is the way through.
        g.op_set_param(q, 0, 0.0).unwrap();
        let ev = s.on_intent(&mut g, GestureIntent::Activate { pos: tap }, &layout, v, &ctx());
        assert_eq!(g.node(q).unwrap().param_value(1), Some(4.0), "Custom: the key edits the mask");
        assert!(ev.iter().any(|e| matches!(e, CanvasEvent::Applied(_))), "{ev:?}");
    }
}
