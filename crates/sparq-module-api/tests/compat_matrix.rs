//! The compat-matrix drift gate (contract v1 increment — the debt carried since WO-007).
//!
//! The connection rules live in two places: `docs/api/compat-matrix.toml` (the table the canvas
//! and the ADR-005 addendum treat as the source of truth) and `src/port.rs` (the compiled
//! `connect_*` functions the canvas affordance layer AND the executor both call — one copy in
//! code, by design). Two copies of anything can drift, and the drift is invisible until a patch
//! refuses to connect on screen for a reason the host happily accepted, or vice versa.
//!
//! This file is the pin between them. It parses the TOML with the crate's OWN parser (the first
//! code consumer the table was written for) and asserts, cell by cell:
//!
//! * the port-type vocabulary, verdict vocabulary and adapter ids are the same closed sets;
//! * every `same_type` case count is what this gate last reviewed — a new or deleted cell fails
//!   here until a human updates the representatives below;
//! * every audio/cv `when` case is REPRESENTED: the table below maps each case string to
//!   concrete `(src, dst, phase)` inputs and the verdict `port.rs` must return for them;
//! * the cross-type adapters are the four ADR-005 names, and each `connect_cross` verdict
//!   offers the module id the table says;
//! * the gap register's statuses move only on purpose.
//!
//! # Why the mirror is pinned rather than deleted (the WO-007 debt, answered honestly)
//!
//! The debt line said "read at discovery, delete the mirror". Full deletion needs the table's
//! `when` cells to be machine predicates; several are prose ("fan-out: one cv output -> many cv
//! inputs", "src.event_kinds subset_of dst.event_kinds") and two are shape rules, not pair rules
//! — no interpreter can evaluate them against `(src, dst)` alone without inventing a query
//! language the ratified artefact does not speak. Restructuring the table is a review-packet
//! change (`[meta] reviewed = false` is still pending), not an implementer's side quest — so this
//! increment does the strongest honest thing: one compiled copy (`port.rs`, consumed by the UI
//! and the executor), one data copy (the table), and THIS gate making silent disagreement a
//! failed build. The first run of this gate found two real drifts and one ordering question —
//! defects #80, #81 and #82 in `PHASE0-WORKORDERS.md` §2.2 — which is the argument that the gate
//! earns its keep.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_module_api::error::CodeKind;
use sparq_module_api::port::{
    connect_audio, connect_cross, connect_cv, Adapter, ChannelSet, CvInterp, CvRange, CvReduce,
    Phase, PortType, Verdict,
};
use sparq_module_api::toml::{self, Table, Value};

/// The table, as bytes on disk — read relative to the crate so the sandbox's path rewriting
/// cannot touch it (the tools/ discipline).
fn matrix() -> Table {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/api/compat-matrix.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    toml::parse(&text).unwrap_or_else(|e| panic!("the matrix no longer parses: {e}"))
}

fn s<'a>(t: &'a Table, key: &str) -> &'a str {
    t.get(key).and_then(Value::as_str).unwrap_or_else(|| panic!("missing string `{key}`"))
}

fn cases<'a>(m: &'a Table, ty: &str) -> Vec<&'a Table> {
    let entries = m.tables("same_type").unwrap();
    let entry = entries
        .into_iter()
        .find(|t| s(t, "type") == ty)
        .unwrap_or_else(|| panic!("no same_type entry for {ty}"));
    entry.tables("cases").unwrap_or_default()
}

fn whens(cs: &[&Table]) -> Vec<String> {
    cs.iter().map(|c| s(c, "when").to_string()).collect()
}

// ---------------------------------------------------------------- vocabulary

#[test]
fn the_port_vocabulary_is_the_closed_set_of_six_with_matching_thread_rules() {
    let m = matrix();
    let pt = m.get("port_types").and_then(Value::as_table).unwrap();
    let names: Vec<&str> = pt.keys().collect();
    assert_eq!(names.len(), 6, "six port types, and nothing else is ever a port type");
    for n in &names {
        let parsed = PortType::parse(n).unwrap_or_else(|| panic!("{n} is not in the compiled set"));
        let row = pt.get(n).and_then(Value::as_table).unwrap();
        let audio_thread = row.get("audio_thread").and_then(Value::as_bool).unwrap();
        assert_eq!(
            parsed.audio_thread(),
            audio_thread,
            "{n}: the table and PortType::audio_thread disagree about the audio thread"
        );
    }
    let meta = m.get("meta").and_then(Value::as_table).unwrap();
    assert_eq!(meta.get("closed_set").and_then(Value::as_bool), Some(true));
}

#[test]
fn the_verdict_vocabulary_is_the_five_the_canvas_renders() {
    let m = matrix();
    let v = m.get("verdicts").and_then(Value::as_table).unwrap();
    let mut names: Vec<&str> = v.keys().collect();
    names.sort_unstable();
    assert_eq!(names, ["adapter", "compatible", "conditional", "conversion", "refused"]);
    // `conditional` is the one verdict with no direct code variant: a conditional cell resolves
    // to Compatible or Refused once the condition is evaluated (channel counts, M equality, the
    // prepared resolution). The representatives below pin both outcomes per conditional cell.
}

#[test]
fn meta_names_the_adr_and_the_schema() {
    let m = matrix();
    let meta = m.get("meta").and_then(Value::as_table).unwrap();
    assert_eq!(s(meta, "schema"), "sparq.compat-matrix");
    assert_eq!(s(meta, "adr"), "005");
    assert_eq!(s(meta, "adr_status"), "accepted");
}

// ---------------------------------------------------------------- same-type cells

#[test]
fn every_same_type_entry_exists_with_the_reviewed_case_count() {
    // The counts are the drift tripwire: adding or removing a cell in the table fails here until
    // a human adds or removes its representative below. That is the gate doing its job.
    let m = matrix();
    let entries = m.tables("same_type").unwrap();
    let got: Vec<(&str, usize)> = entries
        .iter()
        .map(|t| (s(t, "type"), t.tables("cases").map(|c| c.len()).unwrap_or(0)))
        .collect();
    assert_eq!(
        got,
        vec![("audio", 8), ("cv", 5), ("event", 3), ("data", 3), ("gpu", 2), ("atom", 0)],
        "the table's shape changed — update the representatives in this gate, deliberately"
    );
}

/// One represented case: the table's exact `when` string, concrete inputs, and the verdict the
/// compiled matrix must return. Conditional cells carry several rows (both outcomes).
struct Rep {
    when: &'static str,
    src: ChannelSet,
    dst: ChannelSet,
    phase: Phase,
    want: Verdict,
}

#[test]
fn every_audio_cell_is_represented_and_the_code_agrees_with_the_table() {
    let reps = [
        Rep { when: "src.channel_set == dst.channel_set", src: ChannelSet::Stereo, dst: ChannelSet::Stereo, phase: Phase::One, want: Verdict::Compatible },
        Rep { when: "src.channel_set == 'mono' && dst.channel_set != 'mono'", src: ChannelSet::Mono, dst: ChannelSet::Quad, phase: Phase::One, want: Verdict::Compatible },
        Rep { when: "src.channel_set != 'mono' && dst.channel_set == 'mono'", src: ChannelSet::Quad, dst: ChannelSet::Mono, phase: Phase::One, want: Verdict::Conversion },
        Rep { when: "src.channel_set startswith 'ambisonics:' && !(dst.channel_set startswith 'ambisonics:')", src: ChannelSet::Ambisonics(2), dst: ChannelSet::Stereo, phase: Phase::Five, want: Verdict::Adapter(Adapter::HoaDecode) },
        Rep { when: "!(src.channel_set startswith 'ambisonics:') && dst.channel_set startswith 'ambisonics:'", src: ChannelSet::Stereo, dst: ChannelSet::Ambisonics(2), phase: Phase::Five, want: Verdict::Adapter(Adapter::HoaEncode) },
        // conditional: "M must be equal" — both outcomes, as the cell promises.
        Rep { when: "src.channel_set startswith 'raw:' && dst.channel_set startswith 'raw:'", src: ChannelSet::Raw(4), dst: ChannelSet::Raw(4), phase: Phase::One, want: Verdict::Compatible },
        Rep { when: "src.channel_set startswith 'raw:' && dst.channel_set startswith 'raw:'", src: ChannelSet::Raw(4), dst: ChannelSet::Raw(8), phase: Phase::One, want: Verdict::Refused },
        // conditional: resolved at prepare — the connect-time verdict can only be Compatible.
        Rep { when: "src.channel_set == 'variable' || dst.channel_set == 'variable'", src: ChannelSet::Variable, dst: ChannelSet::Stereo, phase: Phase::One, want: Verdict::Compatible },
        // conditional: "K must equal J; otherwise convert via spa/objects" — both outcomes.
        Rep { when: "src.channel_set == 'objects:K' && dst.channel_set == 'objects:J'", src: ChannelSet::Objects(4), dst: ChannelSet::Objects(4), phase: Phase::Five, want: Verdict::Compatible },
        Rep { when: "src.channel_set == 'objects:K' && dst.channel_set == 'objects:J'", src: ChannelSet::Objects(4), dst: ChannelSet::Objects(8), phase: Phase::Five, want: Verdict::Adapter(Adapter::Objects) },
    ];
    let m = matrix();
    let cs = cases(&m, "audio");
    let table_whens = whens(&cs);
    let rep_whens: Vec<&str> = reps.iter().map(|r| r.when).collect();
    for w in &table_whens {
        assert!(rep_whens.contains(&w.as_str()), "table cell without a representative: {w}");
    }
    for r in &reps {
        assert!(
            table_whens.iter().any(|w| w == r.when),
            "representative without a table cell: {}",
            r.when
        );
        let got = connect_audio(r.src, r.dst, r.phase);
        assert_eq!(
            got,
            r.want,
            "cell {:?} → {:?}: table and code disagree",
            r.when,
            (r.src, r.dst)
        );
    }
    // The default cell: two concrete non-mono layouts the cases do not name → refused, which is
    // what the default's error_code (E-CHANNELSET-INCOMPATIBLE) reports at the connect layer.
    assert_eq!(connect_audio(ChannelSet::Stereo, ChannelSet::Quad, Phase::One), Verdict::Refused);
}

#[test]
fn the_mono_fanout_case_precedes_the_spatial_case_in_both_copies_flagged_for_review() {
    // Defect #82, pinned as CONSISTENCY, not correctness: the table lists the mono fan-out case
    // before the ambisonics cases, and the code's match arms follow the same order — so
    // mono → ambisonics:N is a "compatible fan-out" rather than an encoder insertion. Whether
    // that is musically right (a mono channel replicated is NOT a valid AmbiX bed) is a question
    // for the matrix review the pending `reviewed = false` already owes; both copies must move
    // together, and this test is where they are pinned until then.
    assert_eq!(
        connect_audio(ChannelSet::Mono, ChannelSet::Ambisonics(2), Phase::Five),
        Verdict::Compatible
    );
    let m = matrix();
    let cs = cases(&m, "audio");
    let order: Vec<&str> = cs.iter().map(|c| s(c, "when")).collect();
    let mono = order.iter().position(|w| w.contains("== 'mono' &&")).unwrap();
    let ambi = order
        .iter()
        .position(|w| w.contains("startswith 'ambisonics:'") && w.starts_with("src"))
        .unwrap();
    assert!(mono < ambi, "the table's case order changed — defect #82's pin must be re-reviewed");
}

#[test]
fn the_cv_cells_match_the_compiled_rules_and_the_declared_domains() {
    let m = matrix();
    let cs = cases(&m, "cv");
    let by_when = |frag: &str| -> &Table {
        cs.iter()
            .copied()
            .find(|c| s(c, "when").contains(frag))
            .unwrap_or_else(|| panic!("no cv cell matching {frag}"))
    };

    // Range mismatch (G2): refused at Phase 0, util/range offered from Phase 1.
    let range_cell = by_when("src.range != dst.range");
    assert_eq!(s(range_cell, "adapter"), "util/range");
    assert_eq!(
        s(range_cell, "adapter"),
        Adapter::Range.module_id(),
        "the offer names the module the code would insert"
    );
    assert_eq!(connect_cv(CvRange::Bipolar, CvRange::Unipolar, Phase::Zero), Verdict::Refused);
    assert_eq!(
        connect_cv(CvRange::Bipolar, CvRange::Unipolar, Phase::One),
        Verdict::Adapter(Adapter::Range)
    );
    assert_eq!(connect_cv(CvRange::Unipolar, CvRange::Unipolar, Phase::Zero), Verdict::Compatible);

    // Fan-out free, fan-in through util/mixer (the explicit merge — no implicit summing).
    assert_eq!(s(by_when("fan-out"), "verdict"), "compatible");
    let fan_in = by_when("fan-in");
    assert_eq!(s(fan_in, "verdict"), "adapter");
    assert_eq!(s(fan_in, "adapter"), Adapter::Merge.module_id());

    // G3: the reduce domain in the table's condition text is exactly CvReduce's compiled domain,
    // and the promised default is the compiled default. "Declared in the manifest, never in
    // code" — the executor reads Port::cv_reduce; nothing may re-decide it.
    let reduce = by_when("src.rate == 'audio' && dst.rate == 'block'");
    let cond = s(reduce, "condition");
    for name in ["last", "first", "mean", "min", "max", "peak"] {
        assert!(cond.contains(name), "the table's cv_reduce domain lost `{name}`: {cond}");
        assert!(CvReduce::parse(name).is_some(), "`{name}` is in the table but does not compile");
    }
    for name in ["last", "first", "mean", "min", "max", "peak"] {
        let r = CvReduce::parse(name).unwrap();
        assert_eq!(r.as_str(), name, "the spelling round-trips");
        assert!(cond.contains(r.as_str()), "the compiled spelling is absent from the cell: {cond}");
    }
    assert!(cond.contains("default last"), "{cond}");
    assert_eq!(CvReduce::default(), CvReduce::Last, "the compiled default IS the table's default");

    // G4: same pin for the interpolation domain.
    let interp = by_when("src.rate == 'block' && dst.rate == 'audio'");
    let cond = s(interp, "condition");
    for name in ["hold", "linear", "spline"] {
        assert!(cond.contains(name), "the table's cv_interp domain lost `{name}`: {cond}");
        assert!(CvInterp::parse(name).is_some(), "`{name}` is in the table but does not compile");
    }
    assert!(cond.contains("default hold"), "{cond}");
    assert_eq!(CvInterp::default(), CvInterp::Hold);
}

#[test]
fn the_event_cells_pin_the_free_fanin_and_its_tie_break() {
    let m = matrix();
    let cs = cases(&m, "event");
    let subset = cs.iter().copied().find(|c| s(c, "when").contains("subset_of")).unwrap();
    assert_eq!(s(subset, "verdict"), "compatible");
    let fan_in = cs.iter().copied().find(|c| s(c, "when").contains("multiplicity")).unwrap();
    assert_eq!(s(fan_in, "verdict"), "compatible", "event fan-in is free (G5), unlike cv's");
    let tie = s(fan_in, "tie_break");
    assert!(tie.contains("connection id") && tie.contains("insertion order"), "{tie}");
    // The behaviour half of this cell is pinned where it runs: tests/contract_v1.rs
    // (events_from_two_sources_merge_presorted_with_the_matrix_tie_break) reproduces exactly
    // this order — host rank, then connection id, then insertion — and the executor's refusal
    // sentence cites the same E-EVENTKIND-UNACCEPTED spelling the default cell declares:
    let entries = m.tables("same_type").unwrap();
    let event = entries.iter().find(|t| s(t, "type") == "event").unwrap();
    let default = event.get("default").and_then(Value::as_table).unwrap();
    assert_eq!(s(default, "error_code"), "E-EVENTKIND-UNACCEPTED");
}

#[test]
fn the_data_gpu_and_atom_cells_hold_their_declared_rules() {
    let m = matrix();
    let entries = m.tables("same_type").unwrap();
    let data = entries.iter().find(|t| s(t, "type") == "data").unwrap();
    let default = data.get("default").and_then(Value::as_table).unwrap();
    assert_eq!(s(default, "verdict"), "refused");
    // The one matrix error code that IS a manifest-load code in the compiled catalogue:
    assert_eq!(s(default, "error_code"), CodeKind::SchemaUnregistered.prefix());
    let mismatch = data
        .tables("cases")
        .unwrap()
        .into_iter()
        .find(|c| s(c, "when").contains("version !="))
        .unwrap();
    assert_eq!(
        s(mismatch, "verdict"),
        "conditional",
        "G6: loads, flagged stale — never a silent migration"
    );
    assert!(
        s(mismatch, "also_required").contains("journal"),
        "the badge AND the journal entry are mandatory"
    );

    let gpu = entries.iter().find(|t| s(t, "type") == "gpu").unwrap();
    let gcase = gpu
        .tables("cases")
        .unwrap()
        .into_iter()
        .find(|c| c.get("gap").and_then(Value::as_str) == Some("G7"))
        .unwrap();
    assert!(s(gcase, "status").starts_with("DEFERRED"), "G7 moves only through the review packet");
    assert!(s(gcase, "interim_rule").contains("no gpu module may be T2 or T3"));

    let atom = entries.iter().find(|t| s(t, "type") == "atom").unwrap();
    let atom_default = atom.get("default").and_then(Value::as_table).unwrap();
    assert_eq!(s(atom_default, "verdict"), "compatible");
    assert!(!PortType::Atom.audio_thread(), "and never on the audio thread — the code agrees");
}

// ---------------------------------------------------------------- cross-type + adapters + gaps

#[test]
fn the_cross_type_adapters_are_the_four_the_adr_names_with_the_ids_the_code_inserts() {
    let m = matrix();
    let ct = m.get("cross_type").and_then(Value::as_table).unwrap();
    let default = ct.get("default").and_then(Value::as_table).unwrap();
    assert_eq!(
        s(default, "verdict"),
        "refused",
        "same type required; everything else is an exception"
    );
    let rows = ct.get("adapters").and_then(Value::as_array).unwrap();
    assert_eq!(
        rows.len(),
        4,
        "four adapters, no more: an adapter that is not named does not exist"
    );
    let mut seen: Vec<(&str, &str, &str)> = Vec::new();
    for row in rows {
        let t = row.as_table().unwrap();
        seen.push((s(t, "from"), s(t, "to"), s(t, "adapter")));
    }
    // (from, to, table's module id, the code's adapter, the phase it first exists)
    let expect: [(&str, &str, &str, Verdict, Phase); 4] = [
        ("data", "cv", "dat/mapper", Verdict::Adapter(Adapter::Mapper), Phase::Five),
        ("cv", "audio", "util/offset", Verdict::Adapter(Adapter::Offset), Phase::One),
        ("audio", "cv", "ana/rms", Verdict::Adapter(Adapter::Analyser), Phase::One),
        ("event", "cv", "util/gate-to-cv", Verdict::Adapter(Adapter::GateToCv), Phase::One),
    ];
    for (from, to, id, want, phase) in expect {
        assert!(
            seen.contains(&(from, to, id)),
            "the table lost/renamed the {from}→{to} adapter `{id}`: {seen:?}"
        );
        let got =
            connect_cross(PortType::parse(from).unwrap(), PortType::parse(to).unwrap(), phase);
        assert_eq!(got, want, "{from}→{to} at {phase:?}");
        if let Verdict::Adapter(a) = want {
            assert_eq!(a.module_id(), id, "the offer must name the module it inserts");
        }
    }
    // Anything unnamed stays refused at every phase.
    assert_eq!(connect_cross(PortType::Atom, PortType::Audio, Phase::Five), Verdict::Refused);
    assert_eq!(connect_cross(PortType::Gpu, PortType::Data, Phase::Five), Verdict::Refused);
    assert_eq!(connect_cross(PortType::Event, PortType::Audio, Phase::Five), Verdict::Refused);
}

#[test]
fn every_adapter_id_named_anywhere_in_the_table_is_one_the_code_can_offer() {
    // Collects every `adapter = "..."` string in the file (same-type cases and cross-type rows)
    // and compares the SET against the compiled vocabulary — so a module id that exists only in
    // one of the two copies fails here.
    let text = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/api/compat-matrix.toml");
        std::fs::read_to_string(path).unwrap()
    };
    let mut table_ids: Vec<&str> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("adapter = \"") {
            if let Some(id) = rest.strip_suffix('"') {
                table_ids.push(id);
            }
        }
    }
    table_ids.sort_unstable();
    table_ids.dedup();
    let mut code_ids: Vec<&str> = [
        Adapter::Range,
        Adapter::Offset,
        Adapter::Analyser,
        Adapter::GateToCv,
        Adapter::Mapper,
        Adapter::Merge,
        Adapter::HoaEncode,
        Adapter::HoaDecode,
        Adapter::Objects,
    ]
    .iter()
    .map(|a| a.module_id())
    .collect();
    code_ids.sort_unstable();
    code_ids.dedup();
    assert_eq!(table_ids, code_ids, "the adapter vocabularies drifted (defect #80's class)");
}

#[test]
fn the_gap_register_moves_only_on_purpose() {
    let m = matrix();
    let gaps = m.get("gaps").and_then(Value::as_table).unwrap();
    let mut names: Vec<&str> = gaps.keys().collect();
    names.sort_unstable();
    assert_eq!(names, ["G1", "G2", "G3", "G4", "G5", "G6", "G7"]);
    for g in ["G1", "G2", "G3", "G4", "G5", "G6"] {
        let t = gaps.get(g).and_then(Value::as_table).unwrap();
        assert!(
            s(t, "status").starts_with("DECIDED"),
            "{g} was re-opened — that is a review-packet event"
        );
    }
    let g7 = gaps.get("G7").and_then(Value::as_table).unwrap();
    assert!(
        s(g7, "status").starts_with("DEFERRED"),
        "G7 resolved? Then port.rs's Phase gate and this pin move together"
    );
}
