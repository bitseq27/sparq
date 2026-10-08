//! The connection verdict: may this wire exist, and if not, why — in words, with a remedy.
//!
//! The rule that governs this file is the one WO-007 shipped: **the compatibility matrix has one
//! copy.** So the type/range verdict is delegated to `sparq-module-api`'s own [`connect_audio`],
//! [`connect_cv`] and [`connect_cross`] — the compiled form of `docs/api/compat-matrix.toml`. What
//! this file adds is the canvas-level concerns the pure matrix does not own: direction (out→in),
//! duplicate and cycle rejection, the single-input replacement rule, and the availability gate
//! that keeps ADR-005's promise — *"the canvas may never offer a converter that does not exist"*
//! (defect #58).
//!
//! Every refusal is a [`Rejection`] carrying a sentence and a remedy, because on stage "my touch
//! did nothing" must never be a mystery (input-model §3).

use crate::canvas::model::{Graph, NodeSpec, Op, PortRef};
use crate::geom::Vec2;
use sparq_module_api::port::{
    connect_audio, connect_cross, connect_cv, Adapter, ChannelSet, Direction, Multiplicity, Phase,
    PortType, Verdict,
};

/// Why a connection was refused, stated for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejection {
    /// A sentence with a remedy, e.g. "output → output: drag from an output to an input."
    pub reason: String,
}

impl Rejection {
    /// A rejection from a reason string.
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

/// What a connection attempt produced.
// Targeted allow (the clippy.toml house pattern), measured not hoped: `Connected` carries an
// inline `Op`, whose largest variant (`RemoveNode`) owns a whole `Node`. WO-020 INC6's ruled
// `Node.size: Option<Vec2>` (plan D15) grew `Node` 184→200 bytes, which moved this variant's
// difference over `Refused` from 187 to 203 — three bytes past the lint's 200. The outcome is
// control-plane data at gesture rate (one per connection attempt, never on the audio path), so
// the stack copy the lint guards costs nothing here; boxing the `Op` to satisfy the heuristic
// would re-shape a public API across three crates for no measured gain.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectOutcome {
    /// The connection is legal; apply this op (already applied to the graph by
    /// [`resolve`] — the op is returned so the caller can push it on the undo stack).
    Connected {
        /// The op that was applied (a single [`Op::AddWire`] or a replacement/adapter
        /// [`Op::Batch`]).
        op: Op,
        /// True when the matrix required a conversion (multi→mono sum): the wire draws a warning
        /// hairline, the one silent conversion the plan documents (§5.4).
        conversion: bool,
        /// True when an occupied single input had its wire replaced rather than added.
        replaced: bool,
        /// Set when an adapter module was inserted to bridge the types.
        adapter: Option<Adapter>,
    },
    /// The connection is illegal; here is why, in words.
    Refused(Rejection),
}

/// A cheap, **non-mutating** verdict used to glow or dim candidate ports while a wire is being
/// dragged. It deliberately skips the cycle and duplicate checks (a cycle-forming port still
/// glows, then refuses with words on drop) so the per-frame affordance pass over every port stays
/// cheap; the authoritative check is [`resolve`] at drop time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Preview {
    /// Connects directly — glow.
    Compatible,
    /// Connects with a documented conversion (multi→mono) — glow, warning hairline.
    Conversion,
    /// Needs an adapter; `true` when that adapter is installed and would be offered — glow with a
    /// hint, else dim.
    Adapter(bool),
    /// Will not connect (wrong direction, refused, undecided, or missing adapter) — dim.
    Dim,
}

/// Preview the verdict of dragging from `source` onto `candidate`, without mutating the graph.
/// `source` may be an input or an output; the orientation is normalised here (a wire always runs
/// out → in) exactly as [`crate::canvas::interact`] does before calling [`resolve`].
#[must_use]
pub fn preview(
    graph: &Graph,
    source: PortRef,
    candidate: PortRef,
    ctx: &ConnectContext<'_>,
) -> Preview {
    let (Some(s), Some(d)) = (graph.port(source), graph.port(candidate)) else {
        return Preview::Dim;
    };
    // Normalise orientation: (out, in).
    let (out_port, in_port) = match (s.direction, d.direction) {
        (Direction::Out, Direction::In) => (s, d),
        (Direction::In, Direction::Out) => (d, s),
        _ => return Preview::Dim, // out→out or in→in never connects
    };
    match type_verdict(out_port, in_port, ctx.phase) {
        Verdict::Compatible => Preview::Compatible,
        Verdict::Conversion => Preview::Conversion,
        Verdict::Adapter(a) => Preview::Adapter((ctx.adapter_spec)(a).is_some()),
        Verdict::Refused | Verdict::Undecided => Preview::Dim,
    }
}

/// The host-supplied context a verdict needs: what phase we are in, and which converter modules
/// actually exist to be inserted.
///
/// `adapter_spec` is the availability gate: it returns the [`NodeSpec`] for an [`Adapter`]'s
/// module only if that module is really built and registered. Returning `None` — the increment-1
/// default, since no adapter module ships yet — degrades every "adapter needed" verdict to a
/// refusal with a remedy, which is exactly defect #58's rule made structural.
pub struct ConnectContext<'a> {
    /// The release phase, deciding whether the matrix even offers an adapter.
    pub phase: Phase,
    /// Given an adapter, its module spec if (and only if) it is built and available to insert.
    pub adapter_spec: &'a dyn Fn(Adapter) -> Option<NodeSpec>,
}

impl<'a> ConnectContext<'a> {
    /// A context that offers no adapters (none are built). The honest increment-1 default.
    #[must_use]
    pub fn no_adapters(phase: Phase) -> Self {
        Self { phase, adapter_spec: &|_| None }
    }
}

/// Resolve a drag from `from` to `to`: validate, and on success apply the mutation to `graph` and
/// return the op that did it (for undo). On failure, mutate nothing and return the refusal.
///
/// The order of checks is deliberate — cheapest and most certain first, so the reason the user
/// sees is the *real* blocker, not a downstream symptom:
/// ports exist → direction → not a duplicate → not a cycle → matrix verdict → fan-in policy.
pub fn resolve(
    graph: &mut Graph,
    from: PortRef,
    to: PortRef,
    ctx: &ConnectContext<'_>,
) -> ConnectOutcome {
    // The junction bus (operator ruling 2026-10-01 r3): a mult dot's direction and type are
    // set by its first connection, so the matrix — which judges static ports — never sees
    // these pairs; the bus rules below are the mult's own contract, in words either way.
    let from_mult =
        graph.node(from.node).is_some_and(|n| n.spec.module_id == crate::canvas::MULT_ID);
    let to_mult = graph.node(to.node).is_some_and(|n| n.spec.module_id == crate::canvas::MULT_ID);
    if from_mult || to_mult {
        return resolve_mult(graph, from, to);
    }

    let (Some(src), Some(dst)) = (graph.port(from), graph.port(to)) else {
        return ConnectOutcome::Refused(Rejection::new("no such port — the node may have changed"));
    };
    let (src, dst) = (src.clone(), dst.clone());

    // Direction: a wire runs out → in. Anything else is a mis-drag, said plainly.
    match (src.direction, dst.direction) {
        (Direction::Out, Direction::In) => {},
        (Direction::In, Direction::Out) => {
            return ConnectOutcome::Refused(Rejection::new(
                "that runs input → output — drag from the source's output to the target's input",
            ));
        },
        (Direction::Out, Direction::Out) => {
            return ConnectOutcome::Refused(Rejection::new(
                "output → output cannot connect — an output feeds an input",
            ));
        },
        (Direction::In, Direction::In) => {
            return ConnectOutcome::Refused(Rejection::new(
                "input → input cannot connect — drag from an output instead",
            ));
        },
    }

    // Duplicate: the exact wire already exists.
    if graph.wires().iter().any(|w| w.from == from && w.to == to) {
        return ConnectOutcome::Refused(Rejection::new("already connected"));
    }

    // Cycle: §5.4 keeps the graph acyclic by default; the only legal cycle-closers are delay
    // edges, and no delay module exists yet, so a loop is refused with the reason named.
    if from.node == to.node || graph.reaches(to.node, from.node) {
        return ConnectOutcome::Refused(Rejection::new(
            "that would create a cycle — only a delay module may close a loop, and none is installed",
        ));
    }

    // The scope's display binding (operator ruling 2026-10-01 r3): its x/y inputs accept ANY
    // travelling class — audio, cv, event, data — because the scope consumes NOTHING: the wire
    // names the source whose publication the display reads (the audio thread never sees it;
    // the bridge keeps these edges out of the kernel graph). The matrix governs signal paths;
    // a display binding is not one, so it is decided here, in words either way.
    if graph.node(to.node).is_some_and(|n| n.spec.module_id == crate::canvas::SCOPE_ID) {
        return match src.port_type {
            PortType::Audio | PortType::Cv | PortType::Event | PortType::Data => {
                finish_connect(graph, from, to, dst.multiplicity, false, None)
            },
            other => ConnectOutcome::Refused(Rejection::new(format!(
                "{other} carries no signal a scope can read — gpu/atom payloads never travel"
            ))),
        };
    }

    // The matrix verdict, through the contract's own functions (one copy of the truth).
    let verdict = type_verdict(&src, &dst, ctx.phase);
    match verdict {
        Verdict::Compatible | Verdict::Conversion => {
            let conversion = verdict == Verdict::Conversion;
            finish_connect(graph, from, to, dst.multiplicity, conversion, None)
        },
        Verdict::Adapter(a) => match (ctx.adapter_spec)(a) {
            Some(spec) => insert_adapter(graph, from, to, a, spec),
            None => ConnectOutcome::Refused(Rejection::new(format!(
                "{} → {} needs {} , which is not installed — refused (the canvas never offers a converter that does not exist)",
                src.port_type, dst.port_type, a.module_id()
            ))),
        },
        Verdict::Refused => ConnectOutcome::Refused(Rejection::new(format!(
            "{} → {} is refused by the compatibility matrix — no converter bridges these types",
            src.port_type, dst.port_type
        ))),
        Verdict::Undecided => ConnectOutcome::Refused(Rejection::new(format!(
            "{} → {} has no rule in the compatibility matrix yet (ADR-005 G7) — refused until it is decided",
            src.port_type, dst.port_type
        ))),
    }
}

/// Resolve a CONTROL connection (operator ruling 2026-10-01 r3): a cv output onto a float
/// parameter of `dst` — the small blue dot beside a setting, dragged to from a control source.
/// The verdicts are the matrix's discipline applied to modulation: only a `cv` output carries
/// control (audio would modulate at a rate the block snapshot cannot honour, events are not
/// numbers), only a `float` parameter receives it (a toggle or a menu is a decision, not a
/// voltage), cycles are refused as for any wire, and a parameter already modulated gets its
/// wire REPLACED atomically (one undo restores the old modulation — the single-input rule's
/// sibling, because a knob can only wear one wire the way a `single` port can).
pub fn resolve_param(
    graph: &mut Graph,
    from: PortRef,
    dst: crate::canvas::model::NodeId,
    param: usize,
) -> ConnectOutcome {
    use sparq_module_api::manifest::ParamKind;
    let Some(src) = graph.port(from).cloned() else {
        return ConnectOutcome::Refused(Rejection::new("no such source port"));
    };
    if src.direction != Direction::Out {
        return ConnectOutcome::Refused(Rejection::new(
            "a control wire starts at an OUTPUT — drag from the cv source to the setting",
        ));
    }
    if src.port_type != PortType::Cv {
        return ConnectOutcome::Refused(Rejection::new(format!(
            "{} cannot drive a parameter — only a control (cv) output carries modulation",
            src.port_type
        )));
    }
    let Some(desc) = graph.node(dst).and_then(|n| n.spec.params.get(param)).cloned() else {
        return ConnectOutcome::Refused(Rejection::new("that setting does not exist"));
    };
    if desc.kind != ParamKind::Float {
        return ConnectOutcome::Refused(Rejection::new(format!(
            "`{}` is not a float parameter — a toggle or a menu is a decision, not a voltage",
            desc.name
        )));
    }
    if from.node == dst || graph.reaches(dst, from.node) {
        return ConnectOutcome::Refused(Rejection::new(
            "that would create a cycle — a parameter cannot modulate its own source",
        ));
    }
    if graph.wires().iter().any(|w| w.from == from && w.to.node == dst && w.param == Some(param)) {
        return ConnectOutcome::Refused(Rejection::new("already modulated by that source"));
    }
    let existing =
        graph.wires().iter().find(|w| w.to.node == dst && w.param == Some(param)).map(|w| w.id);
    if let Some(old_id) = existing {
        let mut ops = Vec::new();
        if let Some(rm) = graph.op_remove_wire(old_id) {
            ops.push(rm);
        }
        ops.push(graph.op_add_param_wire(from, dst, param));
        return ConnectOutcome::Connected {
            op: Op::Batch(ops),
            conversion: false,
            replaced: true,
            adapter: None,
        };
    }
    let add = graph.op_add_param_wire(from, dst, param);
    ConnectOutcome::Connected { op: add, conversion: false, replaced: false, adapter: None }
}

/// The junction bus verdict (operator ruling 2026-10-01 r3): role set by the first connection
/// and never reinterpreted while a wire touches the dot; type set by the same wire and never
/// changed mid-stream; ONE source per mult (the other dots copy it — sums are the mixer's
/// job); cycles refused like any wire. Every refusal is a sentence naming the rule, because
/// a dot that silently ignores a drag is a dot the operator stops trusting.
fn resolve_mult(graph: &mut Graph, from: PortRef, to: PortRef) -> ConnectOutcome {
    use crate::canvas::layout::signal_class;
    let (mnode, midx, other, other_is_src) =
        if graph.node(from.node).is_some_and(|n| n.spec.module_id == crate::canvas::MULT_ID) {
            (from.node, from.index, to, false)
        } else {
            (to.node, to.index, from, true)
        };
    if graph.node(other.node).is_some_and(|n| n.spec.module_id == crate::canvas::MULT_ID) {
        return ConnectOutcome::Refused(Rejection::new(
            "two mult dots cannot face each other — connect a dot to a real port",
        ));
    }
    let Some(oport) = graph.port(other).cloned() else {
        return ConnectOutcome::Refused(Rejection::new("no such port — the node may have changed"));
    };
    // The dot's intended role: a wire FROM a real port lands ON the dot (input); a wire TO a
    // real port leaves the dot (output).
    let intended = if other_is_src { Direction::In } else { Direction::Out };
    if let Some(role) = graph.mult_port_role(mnode, midx) {
        if role != intended {
            return ConnectOutcome::Refused(Rejection::new(format!(
                "that dot already carries an {} — a mult dot is one or the other while a wire touches it",
                if role == Direction::In { "input" } else { "output" },
            )));
        }
    }
    // The bus's type is the first connection's type — node-wide, not per-dot (a dot that
    // has never carried a wire inherits the bus, it does not start a second one).
    let oclass = signal_class(&oport);
    let dots = graph.node(mnode).map(|n| n.spec.ports.len()).unwrap_or(0);
    let bus_class = (0..dots)
        .filter(|&k| k != midx)
        .filter_map(|k| graph.mult_defining_port(mnode, k))
        .find_map(|pref| graph.port(pref).map(signal_class));
    if let Some(dclass) = bus_class {
        if dclass != oclass {
            return ConnectOutcome::Refused(Rejection::new(format!(
                "that bus already carries a {dclass:?} signal — a mult bus is one type; the first connection set it",
            )));
        }
    }
    if intended == Direction::In {
        if (0..dots).any(|k| k != midx && graph.mult_port_role(mnode, k) == Some(Direction::In)) {
            return ConnectOutcome::Refused(Rejection::new(
                "this mult already has a source — every other dot copies it; sums are the mixer's job",
            ));
        }
        if graph.reaches(mnode, other.node) {
            return ConnectOutcome::Refused(Rejection::new(
                "that would create a cycle — only a delay module may close a loop",
            ));
        }
    } else if graph.reaches(other.node, mnode) {
        return ConnectOutcome::Refused(Rejection::new(
            "that would create a cycle — only a delay module may close a loop",
        ));
    }
    if graph.wires().iter().any(|w| w.from == from && w.to == to) {
        return ConnectOutcome::Refused(Rejection::new("already connected"));
    }
    let add = graph.op_add_wire(from, to);
    ConnectOutcome::Connected { op: add, conversion: false, replaced: false, adapter: None }
}

/// The matrix verdict for a source→destination port pair, dispatching to the right `connect_*`.
fn type_verdict(
    src: &sparq_module_api::manifest::Port,
    dst: &sparq_module_api::manifest::Port,
    phase: Phase,
) -> Verdict {
    match (src.port_type, dst.port_type) {
        (PortType::Audio, PortType::Audio) => {
            let s = src.channel_set.unwrap_or(ChannelSet::Variable);
            let d = dst.channel_set.unwrap_or(ChannelSet::Variable);
            connect_audio(s, d, phase)
        },
        (PortType::Cv, PortType::Cv) => match (src.cv_range, dst.cv_range) {
            // A validated cv port always carries a range; if one somehow does not, treat the
            // range as matching rather than inventing a refusal (validation is the gate, not us).
            (Some(a), Some(b)) => connect_cv(a, b, phase),
            _ => Verdict::Compatible,
        },
        (a, b) => connect_cross(a, b, phase),
    }
}

/// Build the connecting op, honouring the fan-in policy for an occupied input.
fn finish_connect(
    graph: &mut Graph,
    from: PortRef,
    to: PortRef,
    dst_mult: Multiplicity,
    conversion: bool,
    adapter: Option<Adapter>,
) -> ConnectOutcome {
    // Fan-in policy: a `single` input already fed gets its wire *replaced* (the standard modular
    // re-patch), atomically, so one undo restores the old wire. `multi_in`/`multi_out` accept the
    // additional wire. Fan-out from an output is always free (the matrix says so).
    if dst_mult == Multiplicity::Single {
        // Copy the id so the immutable borrow ends before the mutating calls.
        let existing_id = graph.incoming(to).map(|w| w.id);
        if let Some(old_id) = existing_id {
            let mut ops = Vec::new();
            if let Some(rm) = graph.op_remove_wire(old_id) {
                ops.push(rm);
            }
            ops.push(graph.op_add_wire(from, to));
            return ConnectOutcome::Connected {
                op: Op::Batch(ops),
                conversion,
                replaced: true,
                adapter,
            };
        }
    }
    let add = graph.op_add_wire(from, to);
    ConnectOutcome::Connected { op: add, conversion, replaced: false, adapter }
}

/// Insert an adapter module between `from` and `to`: place it midway, wire source→adapter→target.
///
/// The adapter's own ports are trusted to bridge the types (that is what the matrix vouched for
/// when it returned [`Verdict::Adapter`]); we do not re-validate the two half-wires, or a
/// `util/range` could never connect the bipolar↔unipolar pair it exists to join.
fn insert_adapter(
    graph: &mut Graph,
    from: PortRef,
    to: PortRef,
    adapter: Adapter,
    spec: NodeSpec,
) -> ConnectOutcome {
    // Place the adapter midway between the two nodes (world px), on the snap grid.
    let mid = match (graph.node(from.node), graph.node(to.node)) {
        (Some(a), Some(b)) => Vec2::new((a.pos.x + b.pos.x) / 2.0, (a.pos.y + b.pos.y) / 2.0),
        _ => Vec2::ZERO,
    };
    let in_index = spec.inputs().next().unwrap_or(0);
    let out_index = spec.outputs().next().unwrap_or(0);
    let add_node = graph.op_add_node(spec, mid);
    let adapter_id = match &add_node {
        Op::AddNode(n) => n.id,
        _ => return ConnectOutcome::Refused(Rejection::new("adapter insertion failed internally")),
    };
    // Re-patch: the source now feeds the adapter, and the adapter feeds the target. If the target
    // input was single and occupied by the source, that wire is superseded by the second hop.
    let first = graph.op_add_wire(from, PortRef::new(adapter_id, in_index));
    let superseded = graph.incoming(to).filter(|w| w.from == from).map(|w| w.id);
    let second = match superseded {
        Some(old_id) => {
            let mut ops = Vec::new();
            if let Some(rm) = graph.op_remove_wire(old_id) {
                ops.push(rm);
            }
            ops.push(graph.op_add_wire(PortRef::new(adapter_id, out_index), to));
            Op::Batch(ops)
        },
        None => graph.op_add_wire(PortRef::new(adapter_id, out_index), to),
    };
    ConnectOutcome::Connected {
        op: Op::Batch(vec![add_node, first, second]),
        conversion: false,
        replaced: false,
        adapter: Some(adapter),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::canvas::model::{NodeSpec, UndoStack};
    use sparq_module_api::manifest::Port;
    use sparq_module_api::port::{ChannelSet, CvRange, CvRate, Direction, Multiplicity, PortType};

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
    fn node(ports: Vec<Port>, name: &str) -> NodeSpec {
        NodeSpec::new(format!("sparq/{name}"), name, ports)
    }

    // sine: mono out. gain: stereo in, stereo out. rms: stereo in, cv(unipolar) out.
    fn sine() -> NodeSpec {
        node(vec![audio("out", Direction::Out, ChannelSet::Mono)], "sine")
    }
    fn gain() -> NodeSpec {
        node(
            vec![
                audio("in", Direction::In, ChannelSet::Stereo),
                audio("out", Direction::Out, ChannelSet::Stereo),
            ],
            "gain",
        )
    }
    fn ctx0() -> ConnectContext<'static> {
        ConnectContext::no_adapters(Phase::Zero)
    }

    #[test]
    fn mono_out_to_stereo_in_fans_out_and_connects() {
        let mut g = Graph::new();
        let s = g.op_add_node(sine(), Vec2::ZERO);
        let gain = g.op_add_node(gain(), Vec2::new(300.0, 0.0));
        let (sid, gid) = (nid(&s), nid(&gain));
        // sine.out is index 0; gain.in is index 0.
        let out = resolve(&mut g, PortRef::new(sid, 0), PortRef::new(gid, 0), &ctx0());
        assert!(matches!(out, ConnectOutcome::Connected { .. }), "mono → stereo fans out");
        assert_eq!(g.wire_count(), 1);
    }

    #[test]
    fn output_to_output_is_refused_in_words() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain(), Vec2::ZERO);
        let b = g.op_add_node(gain(), Vec2::new(300.0, 0.0));
        // gain.out is index 1 on both.
        let out = resolve(&mut g, PortRef::new(nid(&a), 1), PortRef::new(nid(&b), 1), &ctx0());
        match out {
            ConnectOutcome::Refused(r) => assert!(r.reason.contains("output → output")),
            _ => panic!("expected a refusal"),
        }
        assert_eq!(g.wire_count(), 0);
    }

    #[test]
    fn audio_to_cv_is_refused_at_phase_zero() {
        // audio → cv is Adapter(Analyser) at Phase≥1, which the matrix degrades to Refused at
        // Phase Zero — and no adapter is installed anyway. Either gate refuses it, in words.
        let mut g = Graph::new();
        let gn = g.op_add_node(gain(), Vec2::ZERO);
        let sk = g.op_add_node(
            node(vec![cv("mod", Direction::In, CvRange::Bipolar)], "sink"),
            Vec2::new(300.0, 0.0),
        );
        // gain.out (audio stereo, idx 1) → sink.mod (bipolar cv in, idx 0): cross-type.
        let out = resolve(&mut g, PortRef::new(nid(&gn), 1), PortRef::new(nid(&sk), 0), &ctx0());
        match out {
            ConnectOutcome::Refused(r) => {
                assert!(
                    r.reason.contains("refused") || r.reason.contains("needs"),
                    "reason: {}",
                    r.reason
                );
            },
            other => panic!("expected refusal at Phase Zero, got {other:?}"),
        }
        assert_eq!(g.wire_count(), 0);
    }

    #[test]
    fn a_cycle_is_refused() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain(), Vec2::ZERO);
        let b = g.op_add_node(gain(), Vec2::new(300.0, 0.0));
        let (aid, bid) = (nid(&a), nid(&b));
        let _ = resolve(&mut g, PortRef::new(aid, 1), PortRef::new(bid, 0), &ctx0());
        assert_eq!(g.wire_count(), 1);
        // now b.out → a.in would close the loop
        let out = resolve(&mut g, PortRef::new(bid, 1), PortRef::new(aid, 0), &ctx0());
        match out {
            ConnectOutcome::Refused(r) => assert!(r.reason.contains("cycle")),
            _ => panic!("expected a cycle refusal"),
        }
        assert_eq!(g.wire_count(), 1, "the cycle-closing wire was not added");
    }

    #[test]
    fn connecting_to_an_occupied_single_input_replaces_atomically() {
        let mut g = Graph::new();
        let s1 = g.op_add_node(sine(), Vec2::ZERO);
        let s2 = g.op_add_node(sine(), Vec2::new(0.0, 200.0));
        let gn = g.op_add_node(gain(), Vec2::new(300.0, 0.0));
        let (s1id, s2id, gid) = (nid(&s1), nid(&s2), nid(&gn));
        let mut h = UndoStack::new();
        let first = resolve(&mut g, PortRef::new(s1id, 0), PortRef::new(gid, 0), &ctx0());
        h.push(connected_op(first));
        assert_eq!(g.wire_count(), 1);
        let second = resolve(&mut g, PortRef::new(s2id, 0), PortRef::new(gid, 0), &ctx0());
        let op = connected_op(second);
        assert!(matches!(op, Op::Batch(_)), "a replacement is a batch");
        h.push(op);
        assert_eq!(g.wire_count(), 1, "the single input still has exactly one wire");
        assert!(
            g.incoming(PortRef::new(gid, 0)).is_some_and(|w| w.from.node == s2id),
            "now fed by s2"
        );
        h.undo(&mut g);
        assert!(
            g.incoming(PortRef::new(gid, 0)).is_some_and(|w| w.from.node == s1id),
            "undo restores s1"
        );
    }

    #[test]
    fn an_available_adapter_is_inserted_with_two_wires() {
        // Provide a synthetic util/range spec so the insertion mechanism is exercised even though
        // no adapter ships in increment 1. bipolar cv out → unipolar cv in needs util/range.
        let mut g = Graph::new();
        let src =
            g.op_add_node(node(vec![cv("o", Direction::Out, CvRange::Bipolar)], "src"), Vec2::ZERO);
        let dst = g.op_add_node(
            node(vec![cv("i", Direction::In, CvRange::Unipolar)], "dst"),
            Vec2::new(400.0, 0.0),
        );
        let (sid, did) = (nid(&src), nid(&dst));

        let range_spec = || {
            node(
                vec![
                    cv("in", Direction::In, CvRange::Bipolar),
                    cv("out", Direction::Out, CvRange::Unipolar),
                ],
                "range",
            )
        };
        let ctx = ConnectContext {
            phase: Phase::One,
            adapter_spec: &move |a: Adapter| (a == Adapter::Range).then(range_spec),
        };
        let out = resolve(&mut g, PortRef::new(sid, 0), PortRef::new(did, 0), &ctx);
        match out {
            ConnectOutcome::Connected { adapter: Some(Adapter::Range), .. } => {},
            other => panic!("expected a Range adapter insertion, got {other:?}"),
        }
        assert_eq!(g.node_count(), 3, "the adapter node was inserted");
        assert_eq!(g.wire_count(), 2, "source → adapter → target");
    }

    fn nid(op: &Op) -> crate::canvas::model::NodeId {
        match op {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        }
    }
    fn connected_op(out: ConnectOutcome) -> Op {
        match out {
            ConnectOutcome::Connected { op, .. } => op,
            ConnectOutcome::Refused(r) => panic!("expected a connection, refused: {}", r.reason),
        }
    }
}
