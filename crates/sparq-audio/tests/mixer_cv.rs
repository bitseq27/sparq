//! WO-014 increment 6 gates: `util/mixer` 0.2.0 — the cv merge side the compat-matrix's fan-in
//! cell always named, measured the way the batch files established: properties as arithmetic
//! against hand-computed values, the additive-version discipline pinned (audio indices and
//! goldens untouched), zero allocations counted.
//!
//! The rules this file proves, in words:
//!
//! * **identity default** — an untouched cv side (`cvm0 = 1`, rest 0) is a bit-exact wire from
//!   `cv-0`, the audio side's transparency philosophy at cv scale;
//! * **the merge sums by the DECLARED gains** — hand-computed f64 arithmetic, exact f32 compare;
//! * **the output honours its declared range** — the sum clamps to unipolar 0..1 as the manifest
//!   documents (a module rule, never a silent host transformation);
//! * **an audio-rate source arrives reduced by the port's own `cv_reduce = "mean"`** — checked
//!   against the arithmetic mean of the SOURCE's published buffer in the same block (G3: the
//!   receiver declares, the host performs, the test hand-computes);
//! * **the audio side did not move** — the v0.1.0 identity behaviour re-proven with a full
//!   24-value snapshot (batch-3's 20-value goldens keep passing unchanged: additive by build);
//! * **status decoupling** — `BlockStatus` is the AUDIO contract: `Silenced` with exact-zero
//!   audio while the cv side merges and publishes;
//! * **the fan-in refusal now carries its remedy** — two cv edges into one input still refuse
//!   (no implicit summing, ever) but the sentence names the BUILT merge and its ports;
//! * **zero allocations** — the merge over 1 000 rendered blocks, counted, not commented;
//! * **the snapshot order pin** — params 20..23 are `cvm0..3`, ports 8..12 are the cv side, and
//!   everything before them is byte-for-byte the 0.1.0 order (the "appended LAST" discipline).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::print_stdout)]

use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::modules::register_builtins;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const FRAMES: usize = 64;

/// Manifest port indices (the 0.2.0 order this file pins): audio in 0..3, audio out 4..7,
/// cv in 8..11, cv-out 12.
const CV_IN_0: u32 = 8;
const CV_OUT: u32 = 12;
/// rms publishes `level` on manifest port 1; the lfo publishes its wave on manifest port 1.
const RMS_LEVEL: u32 = 1;
const LFO_OUT: u32 = 1;
/// The svf's `cutoff-mod` cv input is manifest port 2.
const SVF_CUTOFF_MOD: u32 = 2;

/// The 24-value identity snapshot: audio identity matrix, unity trims, cv identity (cv-0 passes).
fn identity24() -> Vec<f32> {
    let mut v = vec![0.0f32; 24];
    for n in 0..4 {
        v[n * 4 + n] = 1.0;
    }
    v[16..20].copy_from_slice(&[1.0, 1.0, 1.0, 1.0]);
    v[20] = 1.0; // cvm0: the untouched cv side is a wire from cv-0
    v
}

fn cfg(ch: usize) -> ExecConfig {
    ExecConfig::new(RATE, FRAMES, ch)
}

fn registry() -> Registry {
    let mut r = Registry::new();
    register_builtins(&mut r).unwrap();
    r
}

fn measure<F: FnOnce()>(f: F) -> u64 {
    let base = start_counting();
    f();
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    made
}

fn build(reg: &Registry, id: &str, params: &[f32]) -> NodeBuild {
    let r = reg.get(id).unwrap_or_else(|| panic!("{id} missing from the registry"));
    NodeBuild {
        module: r.create(),
        manifest: r.manifest().clone(),
        params: ParamSet::new(0, params).unwrap(),
    }
}

fn render_blocks(ex: &mut Executor, master: NodeId, blocks: usize, ch: usize) {
    let mut out = vec![0.0f32; FRAMES * ch];
    for _ in 0..blocks {
        ex.render_block(master, &mut out).unwrap();
    }
}

/// A sine(amp) → rms chain appended to `g`; returns (sine, rms) node ids. The rms `level` is the
/// deterministic unipolar block-rate source the merge gates drive.
fn rms_chain(
    g: &mut Graph,
    reg: &Registry,
    amp: f32,
    builds: &mut Vec<(NodeId, NodeBuild)>,
) -> (NodeId, NodeId) {
    let sine = g.add_node(0);
    let rms = g.add_node(0);
    builds.push((sine, build(reg, "sparq/syn/sine", &[440.0, amp])));
    builds.push((rms, build(reg, "sparq/ana/rms", &[0.0])));
    g.connect(PortRef::new(sine, 0), PortRef::new(rms, 0), EdgeKind::Plain).unwrap();
    (sine, rms)
}

/// A mixer node appended to `g` with the given snapshot; returns its id.
fn mixer(
    g: &mut Graph,
    reg: &Registry,
    params: &[f32],
    builds: &mut Vec<(NodeId, NodeBuild)>,
) -> NodeId {
    let m = g.add_node(0);
    builds.push((m, build(reg, "sparq/util/mixer", params)));
    m
}

#[test]
fn the_identity_default_makes_the_cv_side_a_bit_exact_wire() {
    let reg = registry();
    let mut g = Graph::new();
    let mut builds = Vec::new();
    let (_sine, rms) = rms_chain(&mut g, &reg, 0.5, &mut builds);
    let mix = mixer(&mut g, &reg, &identity24(), &mut builds);
    g.connect(PortRef::new(rms, RMS_LEVEL), PortRef::new(mix, CV_IN_0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(g, builds, cfg(2)).unwrap();
    render_blocks(&mut ex, mix, 20, 2);
    let src = ex.node_cv_block(rms, RMS_LEVEL).expect("rms publishes its level");
    let out = ex.node_cv_block(mix, CV_OUT).expect("the mixer publishes its merge");
    assert!(src > 0.3 && src < 0.4, "the 0.5 sine's rms is the sanity anchor: {src}");
    assert_eq!(out, src, "cvm0 = 1 through the f64 path is a WIRE, bit for bit");
}

#[test]
fn the_merge_sums_by_the_declared_gains_hand_computed() {
    // Two rms chains (amps 0.5 and 0.25) into cv-0/cv-1 with dyadic gains 0.5/0.25: the
    // expected output is the exact f32 of the f64 sum the module documents — same arithmetic,
    // same order, same single write.
    let reg = registry();
    let mut g = Graph::new();
    let mut builds = Vec::new();
    let (_s1, rms1) = rms_chain(&mut g, &reg, 0.5, &mut builds);
    let (_s2, rms2) = rms_chain(&mut g, &reg, 0.25, &mut builds);
    let mut params = identity24();
    params[20] = 0.5; // cvm0
    params[21] = 0.25; // cvm1
    let mix = mixer(&mut g, &reg, &params, &mut builds);
    g.connect(PortRef::new(rms1, RMS_LEVEL), PortRef::new(mix, CV_IN_0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(rms2, RMS_LEVEL), PortRef::new(mix, CV_IN_0 + 1), EdgeKind::Plain)
        .unwrap();
    let mut ex = Executor::build(g, builds, cfg(2)).unwrap();
    render_blocks(&mut ex, mix, 20, 2);
    let l1 = ex.node_cv_block(rms1, RMS_LEVEL).unwrap();
    let l2 = ex.node_cv_block(rms2, RMS_LEVEL).unwrap();
    let out = ex.node_cv_block(mix, CV_OUT).unwrap();
    let want =
        (0.5f64 * f64::from(l1.clamp(0.0, 1.0)) + 0.25f64 * f64::from(l2.clamp(0.0, 1.0))) as f32;
    assert!(l1 > l2 && l2 > 0.1, "the two chains are distinct sources: {l1} vs {l2}");
    assert_eq!(out, want, "the merge arithmetic is the contract");
}

#[test]
fn the_output_clamps_to_the_declared_unipolar_range() {
    let reg = registry();
    let mut g = Graph::new();
    let mut builds = Vec::new();
    let (_sine, rms) = rms_chain(&mut g, &reg, 1.0, &mut builds); // rms ≈ 0.707
    let mut params = identity24();
    params[20] = 2.0; // gain the sum past unity: 0.707 × 2 ≈ 1.414
    let mix = mixer(&mut g, &reg, &params, &mut builds);
    g.connect(PortRef::new(rms, RMS_LEVEL), PortRef::new(mix, CV_IN_0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(g, builds, cfg(2)).unwrap();
    render_blocks(&mut ex, mix, 20, 2);
    let src = ex.node_cv_block(rms, RMS_LEVEL).unwrap();
    let out = ex.node_cv_block(mix, CV_OUT).unwrap();
    assert!(src * 2.0 > 1.0, "the rig really overdrives the sum: {src}");
    assert_eq!(out, 1.0, "the declared range is the ceiling — exactly, not approximately");
}

#[test]
fn an_audio_rate_source_arrives_reduced_by_the_declared_mean() {
    // The lfo publishes an audio-rate wave; the mixer's cv input declares block + mean. The gate
    // hand-computes the mean of the SOURCE's own published buffer for the same block (read via
    // `node_cv_audio`) with the exact arithmetic `CvReduce::Mean` documents — f64 sum, one
    // division, one f32 write — so "the host performs the declared reduce" is measured, not
    // trusted.
    let reg = registry();
    let mut g = Graph::new();
    let mut builds = Vec::new();
    let lfo = g.add_node(0);
    // rate 1 Hz · shape 0 (sine) · depth 1.0 · free-running · division 1 — a full 0..1 swing.
    builds.push((lfo, build(&reg, "sparq/mod/lfo", &[1.0, 0.0, 1.0, 0.0, 1.0])));
    let mix = mixer(&mut g, &reg, &identity24(), &mut builds);
    g.connect(PortRef::new(lfo, LFO_OUT), PortRef::new(mix, CV_IN_0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(g, builds, cfg(2)).unwrap();
    render_blocks(&mut ex, mix, 10, 2);
    let wave = ex.node_cv_audio(lfo, LFO_OUT).expect("the lfo publishes its audio-rate wave");
    assert_eq!(wave.len(), FRAMES);
    let sum: f64 = wave.iter().map(|s| f64::from(*s)).sum();
    let want = (sum / wave.len() as f64) as f32;
    let out = ex.node_cv_block(mix, CV_OUT).unwrap();
    assert!(wave.iter().any(|&v| v > 0.0), "the lfo is actually running");
    assert_eq!(
        out,
        want.clamp(0.0, 1.0),
        "the block value IS the declared mean of the source block"
    );
}

#[test]
fn the_audio_side_renders_unchanged_under_a_full_snapshot() {
    // v0.2.0 with the full 24-value identity snapshot must be the same bit-exact wire the
    // 20-value v0.1.0 snapshot was (batch-3's golden keeps passing untouched — this is the
    // additive claim proven from the new side).
    let reg = registry();
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let mix = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(mix, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (mix, build(&reg, "sparq/util/mixer", &identity24())),
        ],
        cfg(2),
    )
    .unwrap();
    // The reference: the sine alone.
    let mut g2 = Graph::new();
    let s2 = g2.add_node(0);
    let mut ex2 =
        Executor::build(g2, vec![(s2, build(&reg, "sparq/syn/sine", &[440.0, 0.5]))], cfg(2))
            .unwrap();
    let mut a = vec![0.0f32; FRAMES * 2];
    let mut b = vec![0.0f32; FRAMES * 2];
    for _ in 0..50 {
        ex.render_block(mix, &mut a).unwrap();
        ex2.render_block(s2, &mut b).unwrap();
        assert_eq!(a, b, "the identity mixer — cv ports and all — is still a wire");
    }
}

#[test]
fn block_status_is_the_audio_contract_while_the_cv_side_publishes() {
    // No audio connections at all, a live cv merge: the audio outputs are EXACT zeros and the
    // status SAYS Silenced (never silence-by-accident) — and the cv output still publishes the
    // merged value. A cv-only mixer is a legitimate patch citizen, and the two claims do not
    // contradict: BlockStatus describes the audio side.
    let reg = registry();
    let mut g = Graph::new();
    let mut builds = Vec::new();
    let (_sine, rms) = rms_chain(&mut g, &reg, 0.5, &mut builds);
    let mix = mixer(&mut g, &reg, &identity24(), &mut builds);
    g.connect(PortRef::new(rms, RMS_LEVEL), PortRef::new(mix, CV_IN_0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(g, builds, cfg(2)).unwrap();
    let audio = {
        let mut out = vec![0.0f32; FRAMES * 2];
        render_blocks(&mut ex, mix, 5, 2);
        ex.render_block(mix, &mut out).unwrap();
        out
    };
    assert!(audio.iter().all(|&v| v == 0.0), "audio out is exact zeros");
    assert_eq!(
        ex.meter(mix).unwrap().status,
        sparq_module_api::module::BlockStatus::Silenced,
        "and the status says so"
    );
    let src = ex.node_cv_block(rms, RMS_LEVEL).unwrap();
    assert_eq!(ex.node_cv_block(mix, CV_OUT).unwrap(), src, "the cv side merged anyway");
    assert!(src > 0.3, "with a real value: {src}");
}

#[test]
fn the_fan_in_refusal_now_names_the_built_merge_and_its_ports() {
    // The RULE stands — two cv edges into one input refuse, no implicit summing, ever. What
    // changed is the remedy: the sentence names util/mixer's REAL ports, because they exist now.
    let reg = registry();
    let mut g = Graph::new();
    let mut builds = Vec::new();
    let (_s1, rms1) = rms_chain(&mut g, &reg, 0.5, &mut builds);
    let (_s2, rms2) = rms_chain(&mut g, &reg, 0.25, &mut builds);
    let svf = g.add_node(0);
    builds.push((svf, build(&reg, "sparq/flt/svf", &[1000.0, 0.2, 0.0, 0.0])));
    g.connect(PortRef::new(rms1, RMS_LEVEL), PortRef::new(svf, SVF_CUTOFF_MOD), EdgeKind::Plain)
        .unwrap();
    g.connect(PortRef::new(rms2, RMS_LEVEL), PortRef::new(svf, SVF_CUTOFF_MOD), EdgeKind::Plain)
        .unwrap();
    let err = Executor::build(g, builds, cfg(2)).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("util/mixer"), "names the merge module: {msg}");
    assert!(msg.contains("cv-out"), "names the merge's output port: {msg}");
    assert!(msg.contains("cv-0"), "names the merge's inputs: {msg}");
    assert!(msg.contains("no implicit summing"), "the rule itself is stated: {msg}");
}

#[test]
fn the_cv_merge_allocates_nothing_over_a_thousand_blocks() {
    let reg = registry();
    let mut g = Graph::new();
    let mut builds = Vec::new();
    let (_s1, rms1) = rms_chain(&mut g, &reg, 0.5, &mut builds);
    let (_s2, rms2) = rms_chain(&mut g, &reg, 0.25, &mut builds);
    let mut params = identity24();
    params[21] = 0.5;
    let mix = mixer(&mut g, &reg, &params, &mut builds);
    g.connect(PortRef::new(rms1, RMS_LEVEL), PortRef::new(mix, CV_IN_0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(rms2, RMS_LEVEL), PortRef::new(mix, CV_IN_0 + 1), EdgeKind::Plain)
        .unwrap();
    let mut ex = Executor::build(g, builds, cfg(2)).unwrap();
    let mut out = vec![0.0f32; FRAMES * 2];
    // Warm once outside the count (the first block may touch lazily-sized scratch), then measure.
    ex.render_block(mix, &mut out).unwrap();
    let made = measure(|| {
        for _ in 0..1000 {
            ex.render_block(mix, &mut out).unwrap();
        }
    });
    println!("  mixer-cv merge: {made} allocation(s) over 1000 blocks");
    assert_eq!(made, 0, "the merge path allocates nothing per block");
}

#[test]
fn the_snapshot_order_pin_the_appended_last_discipline() {
    // The additive-version contract as data: everything a v0.1.0 patch named keeps its index,
    // and the new side sits strictly after it. A future edit that inserts instead of appends
    // fails here first, in words, instead of silently re-reading somebody's saved snapshot.
    let reg = registry();
    let m = reg.get("sparq/util/mixer").unwrap();
    let man = m.manifest().manifest();
    let ports: Vec<&str> = m.manifest().ports().iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ports,
        vec![
            "in-0", "in-1", "in-2", "in-3", "out-0", "out-1", "out-2", "out-3", "cv-0", "cv-1",
            "cv-2", "cv-3", "cv-out",
        ],
        "ports 0..7 are the v0.1.0 audio side; the cv side appends after"
    );
    let params: Vec<&str> = man.params.iter().map(|p| p.id.as_deref().unwrap()).collect();
    assert_eq!(params.len(), 24);
    assert_eq!(
        &params[..20],
        &[
            "c00", "c01", "c02", "c03", "c10", "c11", "c12", "c13", "c20", "c21", "c22", "c23",
            "c30", "c31", "c32", "c33", "trim0", "trim1", "trim2", "trim3",
        ][..],
        "params 0..19 are the v0.1.0 snapshot, index for index"
    );
    assert_eq!(&params[20..], &["cvm0", "cvm1", "cvm2", "cvm3"][..]);
    // The cv identity default, pinned as data: untouched means cv-0 passes.
    let defs: Vec<f32> = man.params.iter().skip(20).map(|p| p.default.unwrap() as f32).collect();
    assert_eq!(defs, vec![1.0, 0.0, 0.0, 0.0], "the cv default is the identity");
    // And the version says what it is.
    assert_eq!(man.identity.version.as_deref(), Some("0.2.0"));
}

// ---------------------------------------------------------------- operator round 4: the cable
// node's cv side (D15). The trim rides the WIRE — the merge arithmetic downstream is the
// contract this file already pinned, so the gates here are the wire's word: trimmed exactly
// as `v × scale + offset` declares, and identity-exact when the cable node is at rest.

#[test]
fn a_cable_node_trim_rides_the_wire_into_the_merge() {
    // rms ≈ 0.35 through a (0.5, +0.25) trim into cvm0 = 1: the merged word is the trimmed
    // wire word — hand-computed in the wire pass's own f32 order, compared exactly, source
    // and destination read from the SAME rendered block (the per-block ripple trap).
    let reg = registry();
    let mut g = Graph::new();
    let mut builds = Vec::new();
    let (_sine, rms) = rms_chain(&mut g, &reg, 0.5, &mut builds);
    let mix = mixer(&mut g, &reg, &identity24(), &mut builds);
    g.connect(PortRef::new(rms, RMS_LEVEL), PortRef::new(mix, CV_IN_0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(g, builds, cfg(2)).unwrap();
    ex.set_cv_trim(mix, CV_IN_0, 0.5, 0.25).unwrap();
    render_blocks(&mut ex, mix, 20, 2);
    let l = ex.node_cv_block(rms, RMS_LEVEL).expect("rms publishes its level");
    let out = ex.node_cv_block(mix, CV_OUT).expect("the mixer publishes its merge");
    assert!(l > 0.3 && l < 0.4, "the sanity anchor stands: {l}");
    let trimmed = l * 0.5 + 0.25; // the wire pass's arithmetic, same order
    let want = (1.0f64 * f64::from(trimmed.clamp(0.0, 1.0))) as f32; // cvm0 = 1 through the f64 merge
    assert_eq!(out, want, "the merge sums the TRIMMED word");
    assert_ne!(out, l, "…and the trim is audible against the untrimmed wire");
}

#[test]
fn an_identity_cv_trim_is_the_untrimmed_merge_bit_exactly() {
    // Two identical worlds; one calls `set_cv_trim(…, 1.0, 0.0)`. Every merged word AND every
    // rendered sample must be bit-equal: the identity is a BRANCH, which is what lets the
    // bridge ride this door for every trim record without moving the untrimmed goldens.
    let world = |trim: Option<(f32, f32)>| -> Vec<f32> {
        let reg = registry();
        let mut g = Graph::new();
        let mut builds = Vec::new();
        let (_sine, rms) = rms_chain(&mut g, &reg, 0.5, &mut builds);
        let mix = mixer(&mut g, &reg, &identity24(), &mut builds);
        g.connect(PortRef::new(rms, RMS_LEVEL), PortRef::new(mix, CV_IN_0), EdgeKind::Plain)
            .unwrap();
        let mut ex = Executor::build(g, builds, cfg(2)).unwrap();
        if let Some((s, o)) = trim {
            ex.set_cv_trim(mix, CV_IN_0, s, o).unwrap();
        }
        let mut seq = Vec::new();
        let mut out = vec![0.0f32; FRAMES * 2];
        for _ in 0..20 {
            ex.render_block(mix, &mut out).unwrap();
            seq.push(ex.node_cv_block(mix, CV_OUT).unwrap());
            seq.extend_from_slice(&out);
        }
        seq
    };
    let plain = world(None);
    let identity = world(Some((1.0, 0.0)));
    assert_eq!(plain, identity, "identity trim = no trim, bit for bit");
    assert!(plain.iter().any(|&v| v != 0.0), "and the world is not silent");
}
