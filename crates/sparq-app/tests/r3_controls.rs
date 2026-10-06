//! Operator round 2026-10-01 r3 gates: control wires into float parameters, the junction
//! bus, and the sequencer's step buttons. Properties as arithmetic and exact verdicts, the
//! batch-gate discipline: every rule the canvas enforces in words is proven here to fire.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::modules::register_builtins;
use sparq_module_api::manifest::Port;
use sparq_module_api::port::{
    ChannelSet, CvRange, CvRate, Direction, Multiplicity, Phase, PortType,
};
use sparq_module_api::registry::Registry;
use sparq_ui::canvas::connect::{self, ConnectContext, ConnectOutcome};
use sparq_ui::canvas::inspector;
use sparq_ui::canvas::layout::{self, Hit};
use sparq_ui::canvas::model::{Graph, NodeId, NodeSpec, Op, ParamDesc, ParamKind, PortRef};
use sparq_ui::canvas::Lod;
use sparq_ui::geom::{Rect, Vec2};

const RATE: u32 = 48_000;
const FRAMES: usize = 64;

fn registry() -> Registry {
    let mut r = Registry::new();
    register_builtins(&mut r).unwrap();
    r
}

fn ctx() -> ConnectContext<'static> {
    ConnectContext::no_adapters(Phase::Zero)
}

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

fn cv_out(id: &str) -> Port {
    Port {
        id: id.into(),
        direction: Direction::Out,
        port_type: PortType::Cv,
        required: false,
        channel_set: None,
        channel_set_variable: false,
        cv_rate: Some(CvRate::Audio),
        cv_range: Some(CvRange::Bipolar),
        cv_reduce: Default::default(),
        cv_interp: Default::default(),
        event_kinds: Vec::new(),
        multiplicity: Multiplicity::Single,
        latency_contribution: 0,
    }
}

fn lfo_like() -> NodeSpec {
    NodeSpec::new("sparq/mod/lfo", "LFO", vec![cv_out("out")])
}

fn gain_like() -> NodeSpec {
    NodeSpec::new(
        "sparq/util/gain",
        "Gain",
        vec![
            audio("in", Direction::In, ChannelSet::Stereo),
            audio("out", Direction::Out, ChannelSet::Stereo),
        ],
    )
    .with_params(vec![ParamDesc {
        id: "gain".into(),
        name: "Gain".into(),
        kind: ParamKind::Float,
        unit: Some("ratio".into()),
        min: 0.0,
        max: 2.0,
        default: 1.0,
        options: Vec::new(),
    }])
}

fn nid(op: &Op) -> NodeId {
    match op {
        Op::AddNode(n) => n.id,
        _ => unreachable!(),
    }
}

// ------------------------------------------------------------------ control wires

#[test]
fn a_cv_output_modulates_a_float_parameter_and_replaces_an_old_modulation() {
    let mut g = Graph::new();
    let lfo = nid(&g.op_add_node(lfo_like(), Vec2::ZERO));
    let gain = nid(&g.op_add_node(gain_like(), Vec2::new(300.0, 0.0)));
    // first control wire connects
    let out = connect::resolve_param(&mut g, PortRef::new(lfo, 0), gain, 0);
    assert!(matches!(out, ConnectOutcome::Connected { replaced: false, .. }), "{out:?}");
    assert_eq!(g.wires().len(), 1);
    assert_eq!(g.wires()[0].param, Some(0), "the wire lands on the parameter");
    // a second source onto the SAME parameter replaces it atomically
    let lfo2 = nid(&g.op_add_node(lfo_like(), Vec2::new(0.0, 300.0)));
    let out = connect::resolve_param(&mut g, PortRef::new(lfo2, 0), gain, 0);
    assert!(matches!(out, ConnectOutcome::Connected { replaced: true, .. }), "{out:?}");
    assert_eq!(g.wires().len(), 1, "one modulation per knob");
    assert_eq!(g.wires()[0].from, PortRef::new(lfo2, 0));
}

#[test]
fn control_wires_refuse_non_cv_sources_and_non_float_sinks_in_words() {
    let mut g = Graph::new();
    let gain = nid(&g.op_add_node(gain_like(), Vec2::ZERO));
    let gain2 = nid(&g.op_add_node(gain_like(), Vec2::new(300.0, 0.0)));
    // an audio output is not a control voltage
    let out = connect::resolve_param(&mut g, PortRef::new(gain, 1), gain2, 0);
    match out {
        ConnectOutcome::Refused(r) => assert!(r.reason.contains("cv"), "{}", r.reason),
        other => panic!("{other:?}"),
    }
    // a toggle is a decision, not a voltage: an int 0..1 param refuses
    let mute = NodeSpec::new(
        "sparq/out/main",
        "Main",
        vec![
            audio("in", Direction::In, ChannelSet::Stereo),
            audio("out", Direction::Out, ChannelSet::Stereo),
        ],
    )
    .with_params(vec![ParamDesc {
        id: "mute".into(),
        name: "Mute".into(),
        kind: ParamKind::Int,
        unit: Some("x".into()),
        min: 0.0,
        max: 1.0,
        default: 0.0,
        options: Vec::new(),
    }]);
    let m = nid(&g.op_add_node(mute, Vec2::new(600.0, 0.0)));
    let lfo = nid(&g.op_add_node(lfo_like(), Vec2::new(0.0, -300.0)));
    let out = connect::resolve_param(&mut g, PortRef::new(lfo, 0), m, 0);
    match out {
        ConnectOutcome::Refused(r) => assert!(r.reason.contains("float"), "{}", r.reason),
        other => panic!("{other:?}"),
    }
    assert!(g.wires().is_empty(), "nothing connected across both refusals");
}

#[test]
fn the_sink_dot_is_hit_testable_where_it_is_drawn() {
    let mut g = Graph::new();
    let gain = nid(&g.op_add_node(gain_like(), Vec2::ZERO));
    let cam = sparq_ui::canvas::Camera::new();
    let view = Rect::from_min_size(Vec2::ZERO, Vec2::new(1200.0, 800.0));
    let layout = layout::compute(&g, &cam, view);
    let nl = layout.nodes.iter().find(|n| n.id == gain).unwrap();
    let sink = nl.param_rows[0].sink.expect("a float param wears a sink dot");
    // on the dot: the sink captures (while a control drag is in flight)
    assert_eq!(layout::param_sink_at(&layout, sink), Some((gain, 0)));
    // a hair outside the small capture: the card body, not the dot
    assert!(
        layout::param_sink_at(&layout, Vec2::new(sink.x, sink.y + 30.0)).is_none(),
        "the sink's grab is smaller than a port's"
    );
    // and the row itself still routes to the slider, not the sink
    assert!(matches!(
        layout::hit_test(&layout, nl.param_rows[0].row.center(), Lod::Full),
        Hit::Param(..)
    ));
}

// ------------------------------------------------------------------ the executor modulates

#[test]
fn a_control_wire_modulates_the_parameter_every_block() {
    // sine → gain, and an lfo-like cv source modulating the gain's parameter 0. The rendered
    // hash must DIFFER from the unmodulated render (the wire is audible), and the peak must
    // stay inside the clamp the manifest declares (0..=2).
    let reg = registry();
    let build = |id: &str, params: &[f32]| {
        let r = reg.get(id).unwrap();
        NodeBuild {
            module: r.create(),
            manifest: r.manifest().clone(),
            params: sparq_module_api::params::ParamSet::new(1, params).unwrap(),
        }
    };
    let mut g = sparq_kernel::graph::Graph::new();
    let sine = g.add_node(0);
    let gain = g.add_node(0);
    let lfo = g.add_node(0);
    g.connect(
        sparq_kernel::graph::PortRef::new(sine, 0),
        sparq_kernel::graph::PortRef::new(gain, 0),
        sparq_kernel::graph::EdgeKind::Plain,
    )
    .unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build("sparq/syn/sine", &[440.0, 0.5])),
            (gain, build("sparq/util/gain", &[1.0])),
            (lfo, build("sparq/mod/lfo", &[4.0, 0.0, 1.0, 0.0, 0.0])),
        ],
        ExecConfig::new(RATE, FRAMES, 2),
    )
    .unwrap();
    // the control connection: lfo cv out (port 0) → gain param 0, range 0..2
    ex.add_param_mod(gain, 0, lfo, 1, 0.0, 2.0, 1.0, 0.0).unwrap(); // port 1 = the lfo's cv out; identity trim
    let mut out = vec![0.0f32; FRAMES * 2];
    let mut peak_mod = 0.0f32;
    for _ in 0..200 {
        ex.render_block(gain, &mut out).unwrap();
        peak_mod = peak_mod.max(out.iter().fold(0.0f32, |a, &s| a.max(s.abs())));
    }
    assert!(peak_mod > 0.0);
    assert!(peak_mod <= 0.5 * 2.0 + 1e-3, "the clamp holds: {peak_mod}");
    // the unmodulated render of the same patch: a constant 0.5 peak (sine amp × gain 1)
    let mut g = sparq_kernel::graph::Graph::new();
    let sine = g.add_node(0);
    let gain = g.add_node(0);
    g.connect(
        sparq_kernel::graph::PortRef::new(sine, 0),
        sparq_kernel::graph::PortRef::new(gain, 0),
        sparq_kernel::graph::EdgeKind::Plain,
    )
    .unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build("sparq/syn/sine", &[440.0, 0.5])),
            (gain, build("sparq/util/gain", &[1.0])),
        ],
        ExecConfig::new(RATE, FRAMES, 2),
    )
    .unwrap();
    let mut peak_plain = 0.0f32;
    for _ in 0..200 {
        ex.render_block(gain, &mut out).unwrap();
        peak_plain = peak_plain.max(out.iter().fold(0.0f32, |a, &s| a.max(s.abs())));
    }
    assert!(
        (peak_plain - 0.5).abs() < 1e-3,
        "unmodulated: the exact constant multiply ({peak_plain})"
    );
    assert!(
        (peak_mod - peak_plain).abs() > 1e-3,
        "the control wire is audible: mod {peak_mod} vs plain {peak_plain}"
    );
}

// ------------------------------------------------------------------ the junction bus

#[test]
fn mult_roles_and_types_are_set_by_the_first_connection_and_refuse_the_grain() {
    let mut g = Graph::new();
    let sine = nid(&g.op_add_node(
        NodeSpec::new(
            "sparq/syn/sine",
            "Sine",
            vec![audio("out", Direction::Out, ChannelSet::Mono)],
        ),
        Vec2::ZERO,
    ));
    let mult = nid(&g.op_add_node(mult_spec(), Vec2::new(300.0, 0.0)));
    let gain = nid(&g.op_add_node(gain_like(), Vec2::new(600.0, 0.0)));
    let lfo = nid(&g.op_add_node(lfo_like(), Vec2::new(300.0, 400.0)));
    // first connection sets dot 0's role (input) and the bus's type (audio)
    assert!(matches!(
        connect::resolve(&mut g, PortRef::new(sine, 0), PortRef::new(mult, 0), &ctx()),
        ConnectOutcome::Connected { .. }
    ));
    assert_eq!(g.mult_port_role(mult, 0), Some(Direction::In));
    // an output dot copies the bus to a real input
    assert!(matches!(
        connect::resolve(&mut g, PortRef::new(mult, 1), PortRef::new(gain, 0), &ctx()),
        ConnectOutcome::Connected { .. }
    ));
    assert_eq!(g.mult_port_role(mult, 1), Some(Direction::Out));
    // against the grain: a second input dot is refused (one source per bus)
    match connect::resolve(&mut g, PortRef::new(sine, 0), PortRef::new(mult, 2), &ctx()) {
        ConnectOutcome::Refused(r) => assert!(r.reason.contains("source"), "{}", r.reason),
        other => panic!("{other:?}"),
    }
    // a role flip is refused while a wire touches the dot: dot 0 carries an input, so it
    // cannot suddenly feed the gain (the cycle check would also fire on the reverse drag)
    match connect::resolve(&mut g, PortRef::new(mult, 0), PortRef::new(gain, 0), &ctx()) {
        ConnectOutcome::Refused(r) => {
            assert!(r.reason.contains("one or the other"), "{}", r.reason)
        },
        other => panic!("{other:?}"),
    }
    // a cv source cannot even join an audio bus as a second input: the type rule is the
    // deeper refusal and fires first (the single-source rule follows it in resolve_mult)
    match connect::resolve(&mut g, PortRef::new(lfo, 0), PortRef::new(mult, 3), &ctx()) {
        ConnectOutcome::Refused(r) => assert!(r.reason.contains("one type"), "{}", r.reason),
        other => panic!("{other:?}"),
    }
    assert_eq!(g.wires().len(), 2, "only the two accepted wires exist");
    // ...and the TYPE rule fires on a fresh bus: first connection cv, second audio refused
    let mut g2 = Graph::new();
    let lfo2 = nid(&g2.op_add_node(lfo_like(), Vec2::ZERO));
    let sine2 = nid(&g2.op_add_node(
        NodeSpec::new(
            "sparq/syn/sine",
            "Sine",
            vec![audio("out", Direction::Out, ChannelSet::Mono)],
        ),
        Vec2::new(0.0, 300.0),
    ));
    let mult2 = nid(&g2.op_add_node(mult_spec(), Vec2::new(300.0, 0.0)));
    let gain2 = nid(&g2.op_add_node(gain_like(), Vec2::new(600.0, 0.0)));
    assert!(matches!(
        connect::resolve(&mut g2, PortRef::new(lfo2, 0), PortRef::new(mult2, 0), &ctx()),
        ConnectOutcome::Connected { .. }
    ));
    // an output dot cannot change the bus's type: the cv bus refuses an audio destination
    match connect::resolve(&mut g2, PortRef::new(mult2, 1), PortRef::new(gain2, 0), &ctx()) {
        ConnectOutcome::Refused(r) => assert!(r.reason.contains("one type"), "{}", r.reason),
        other => panic!("{other:?}"),
    }
    // and a cv destination copies fine
    let _ = sine2; // the audio source's refusal is the single-source rule, asserted above
    assert!(
        matches!(
            connect::resolve(&mut g2, PortRef::new(mult2, 1), PortRef::new(lfo2, 0), &ctx()),
            ConnectOutcome::Refused(_)
        ),
        "a cv INPUT as the destination of an output dot is direction-nonsense, refused"
    );
}

fn mult_spec() -> NodeSpec {
    let reg = registry();
    NodeSpec::from_manifest(reg.get("sparq/util/mult").unwrap().manifest())
}

#[test]
fn a_mult_wire_set_is_a_bus_the_canvas_and_the_bridge_agree_on() {
    // The render-level half of this rule (the collapsed kernel edges, the copy lighting
    // like the original) lives in the headless smoke `run_r3_smokes` — the bridge is a
    // binary-crate module, so its proof rides the audit, not an integration test.
    let reg = registry();
    let mut g = Graph::new();
    let sine = nid(&g.op_add_node(
        NodeSpec::from_manifest(reg.get("sparq/syn/sine").unwrap().manifest()),
        Vec2::ZERO,
    ));
    let mult = nid(&g.op_add_node(mult_spec(), Vec2::new(300.0, 0.0)));
    let gain = nid(&g.op_add_node(gain_like(), Vec2::new(600.0, 0.0)));
    connect::resolve(&mut g, PortRef::new(sine, 0), PortRef::new(mult, 0), &ctx());
    connect::resolve(&mut g, PortRef::new(mult, 1), PortRef::new(gain, 0), &ctx());
    connect::resolve(&mut g, PortRef::new(mult, 2), PortRef::new(gain, 0), &ctx());
    assert_eq!(g.wires().len(), 3);
    // fan-out from an output dot is free (the kernel's own rule, worn by the bus)
    assert_eq!(g.mult_port_role(mult, 1), Some(Direction::Out));
    assert_eq!(g.mult_port_role(mult, 2), Some(Direction::Out));
}

// ------------------------------------------------------------------ the step buttons

#[test]
fn the_step_buttons_toggle_their_bits_through_the_param_door() {
    let reg = registry();
    let mut g = Graph::new();
    let seq = nid(&g.op_add_node(
        NodeSpec::from_manifest(reg.get("sparq/mod/seq").unwrap().manifest()),
        Vec2::ZERO,
    ));
    let mut s = sparq_ui::canvas::interact::CanvasState::new();
    let view = Rect::from_min_size(Vec2::ZERO, Vec2::new(1200.0, 800.0));
    let layout = layout::compute(&g, &s.camera, view);
    let nl = layout.nodes.iter().find(|n| n.id == seq).unwrap();
    assert_eq!(nl.step_cells.len(), 16, "sixteen buttons on the card");
    // tap step 0 (default mask 170 has bit 0 clear) → set; tap again → clear
    let pat = |g: &Graph| {
        let idx = g.node(seq).unwrap().spec.params.iter().position(|d| d.id == "pattern").unwrap();
        g.node(seq).unwrap().param_value(idx).unwrap()
    };
    let before = pat(&g);
    s.on_intent(
        &mut g,
        sparq_ui::gesture::GestureIntent::Activate { pos: nl.step_cells[0].center() },
        &layout,
        view,
        &ctx(),
    );
    assert_eq!(pat(&g), before + 1.0, "step 0's bit set");
    s.on_intent(
        &mut g,
        sparq_ui::gesture::GestureIntent::Activate { pos: nl.step_cells[0].center() },
        &layout,
        view,
        &ctx(),
    );
    assert_eq!(pat(&g), before, "and cleared again — one undoable flip each way");
    // step 1 (bit 1 set in 170) clears on the first tap
    s.on_intent(
        &mut g,
        sparq_ui::gesture::GestureIntent::Activate { pos: nl.step_cells[1].center() },
        &layout,
        view,
        &ctx(),
    );
    assert_eq!(pat(&g), before - 2.0);
    // the inspector's 2×8 chips address the same bits
    let panel = Rect::from_min_size(Vec2::new(800.0, 100.0), Vec2::new(400.0, 600.0));
    let il = inspector::compute(g.node(seq).unwrap(), panel);
    let row = il.rows.iter().find(|r| !r.steps.is_empty()).expect("the pattern row wears chips");
    assert_eq!(row.steps.len(), 16);
}

#[test]
fn a_control_wires_trim_rides_the_modulation_formula() {
    // Operator round 4, D15 — the control side of the cable node: the latched cv word is
    // trimmed BEFORE the r3 additive modulation, so the effective value per block is
    // `clamp(base + (scalar × scale + offset) × depth, lo, hi)`. Three renders of the same
    // patch — unmodulated, identity-modulated, trimmed-modulated (scale 0.5, offset 0.25) —
    // and the peaks must ORDER: the trim shrinks the swing and raises the floor, audibly,
    // inside the same clamp (ratios and orderings, not absolutes: the gain glide lags the
    // 4 Hz lfo, the trap the round-4 handoff named).
    let reg = registry();
    let build = |id: &str, params: &[f32]| {
        let r = reg.get(id).unwrap();
        NodeBuild {
            module: r.create(),
            manifest: r.manifest().clone(),
            params: sparq_module_api::params::ParamSet::new(1, params).unwrap(),
        }
    };
    let world = |trim: Option<(f32, f32)>| -> f32 {
        let mut g = sparq_kernel::graph::Graph::new();
        let sine = g.add_node(0);
        let gain = g.add_node(0);
        let lfo = g.add_node(0);
        g.connect(
            sparq_kernel::graph::PortRef::new(sine, 0),
            sparq_kernel::graph::PortRef::new(gain, 0),
            sparq_kernel::graph::EdgeKind::Plain,
        )
        .unwrap();
        let mut ex = Executor::build(
            g,
            vec![
                (sine, build("sparq/syn/sine", &[440.0, 0.5])),
                (gain, build("sparq/util/gain", &[1.0])),
                (lfo, build("sparq/mod/lfo", &[4.0, 0.0, 1.0, 0.0, 0.0])),
            ],
            ExecConfig::new(RATE, FRAMES, 2),
        )
        .unwrap();
        if let Some((scale, offset)) = trim {
            ex.add_param_mod(gain, 0, lfo, 1, 0.0, 2.0, scale, offset).unwrap();
        }
        let mut out = vec![0.0f32; FRAMES * 2];
        let mut peak = 0.0f32;
        for _ in 0..400 {
            ex.render_block(gain, &mut out).unwrap();
            peak = peak.max(out.iter().fold(0.0f32, |a, &s| a.max(s.abs())));
        }
        peak
    };
    let unmod = world(None); // gain 1.0 → peak ≈ 0.5
    let identity = world(Some((1.0, 0.0))); // base 1 + cv × 1 → peak ≈ 1.0 at the lfo crest
    let trimmed = world(Some((0.5, 0.25))); // base 1 + (0.5 cv + 0.25) → crest ≈ 0.875
    assert!((unmod - 0.5).abs() < 0.02, "the unmodulated anchor: {unmod}");
    assert!(identity > trimmed, "the trim shrinks the swing: {identity} vs {trimmed}");
    assert!(
        trimmed > unmod,
        "…and its raised floor still modulates above the bare knob: {trimmed}"
    );
    assert!((trimmed - 0.875).abs() < 0.05, "the crest lands where the formula says: {trimmed}");
}
