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

use crate::canvas::camera::{Camera, Lod};
use crate::canvas::connect::{self, ConnectContext, ConnectOutcome};
use crate::canvas::layout::{self, CanvasLayout, Hit};
use crate::canvas::model::{Graph, NodeId, Op, PortRef, UndoStack, WireId};
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

/// Keep a menu of `size` inside `view` given a requested `anchor`.
fn clamp_origin(anchor: Vec2, size: Vec2, view: Rect) -> Vec2 {
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
        }
    }

    /// Which node feeds the listener: the explicit master when set (and still present), else the
    /// default rule — **the highest-id node that has an audio output AND at least one wire**,
    /// preferring one with no outgoing wires (a terminus). The rule exists so the bridge never
    /// guesses silently: every render logs the master it chose. Unconnected spare modules (no
    /// wires at all) are excluded — rendering one would produce silence and confusion.
    #[must_use]
    pub fn resolve_master(&self, graph: &Graph) -> Option<crate::canvas::model::NodeId> {
        use sparq_module_api::port::{Direction, PortType};
        if let Some(m) = self.master {
            if graph.node(m).is_some() {
                return Some(m);
            }
        }
        let has_audio_out = |n: &crate::canvas::model::Node| {
            n.spec
                .ports
                .iter()
                .any(|p| p.direction == Direction::Out && p.port_type == PortType::Audio)
        };
        let wired = |id: crate::canvas::model::NodeId| {
            graph.wires().iter().any(|w| w.from.node == id || w.to.node == id)
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
        // An open menu captures the tap: a row runs its action, anything else closes the menu.
        if let Some(menu) = self.menu.clone() {
            let origin = self.menu_origin(view).unwrap_or(menu.anchor);
            for (i, row) in menu.rows.iter().enumerate() {
                if menu.row_rect(origin, i).contains(pos) {
                    if row.enabled {
                        self.menu = None;
                        let mut ev = vec![CanvasEvent::Menu(false)];
                        ev.extend(self.run_menu_action(graph, menu.target, row.action, view));
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
            Hit::Wire(id) => {
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

    fn open_menu(&mut self, graph: &Graph, pos: Vec2, layout: &CanvasLayout) -> Vec<CanvasEvent> {
        let lod = self.camera.lod();
        let target = match layout::hit_test(layout, pos, lod) {
            Hit::Node(id) => {
                self.selection.clear();
                self.selection.nodes.insert(id);
                MenuTarget::Node(id)
            },
            Hit::Wire(id) => {
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
                MenuRow::row(MenuAction::SelectAll, "SELECT ALL", true),
                MenuRow::row(MenuAction::ZoomFit, "ZOOM TO FIT", true),
                MenuRow::row(MenuAction::RenderWav, "RENDER WAV", true),
                MenuRow::row(MenuAction::Redo, "REDO", self.history.can_redo()),
            ],
        };
        self.menu = Some(MenuState { target, anchor: pos, rows });
        vec![CanvasEvent::Menu(true)]
    }

    fn run_menu_action(
        &mut self,
        graph: &mut Graph,
        target: MenuTarget,
        action: MenuAction,
        view: Rect,
    ) -> Vec<CanvasEvent> {
        match (target, action) {
            (MenuTarget::Node(id), MenuAction::Duplicate) => self.duplicate(graph, id),
            (MenuTarget::Node(id), MenuAction::SetMaster) => {
                self.master = Some(id);
                vec![CanvasEvent::MasterSet(id)]
            },
            (MenuTarget::Empty, MenuAction::RenderWav) => vec![CanvasEvent::RenderWav],
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

    fn drag_start(&mut self, graph: &Graph, pos: Vec2, layout: &CanvasLayout) -> Vec<CanvasEvent> {
        // An open menu is dismissed by a drag that starts outside it.
        self.menu = None;
        let lod = self.camera.lod();
        match layout::hit_test(layout, pos, lod) {
            Hit::Port(pref, dir) => {
                self.interaction = Interaction::Wire(PendingWire {
                    from: pref,
                    from_dir: dir,
                    cursor_screen: pos,
                    hovered: Some(pref),
                });
                vec![CanvasEvent::Note("drawing a wire".to_string())]
            },
            Hit::Node(id) => {
                let locked = graph.node(id).map(|n| n.flags.locked).unwrap_or(false);
                if locked {
                    return vec![CanvasEvent::Refused(
                        "node is locked — unlock it from the long-press menu to move it"
                            .to_string(),
                    )];
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
                vec![CanvasEvent::Selection(self.selection.len())]
            },
            Hit::Wire(id) => {
                self.selection.clear();
                self.selection.wires.insert(id);
                // Wire endpoint re-patch by drag is increment 2; selecting on drag-start is honest.
                vec![
                    CanvasEvent::Selection(1),
                    CanvasEvent::Note("wire re-patch by drag lands in increment 2".to_string()),
                ]
            },
            Hit::Empty => {
                self.interaction =
                    Interaction::Marquee { start_screen: pos, acc_screen: Vec2::ZERO };
                Vec::new()
            },
        }
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
            Interaction::Marquee { start_screen, acc_screen } => {
                self.finish_marquee(graph, start_screen, acc_screen, view)
            },
            Interaction::Idle => Vec::new(),
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
    use crate::canvas::model::NodeSpec;
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
}
