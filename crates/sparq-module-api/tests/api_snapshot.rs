//! WO-007 task 5: the machine-checked API surface snapshot.
//!
//! Acceptance criterion: *"API snapshot test exists and fails on an intentional signature change."*
//!
//! The mechanism is a `const` of function-pointer type for every public entry point. Coercing a
//! function to a `fn` pointer type pins its signature **exactly** — argument types, argument order
//! and return type — so any incompatible change to the contract is a *compile* error in this file,
//! which fails CI before it can reach a module author. No dependency, no codegen, no nightly.
//!
//! Where a receiver is generic over a lifetime (`AudioCtx`), rustc refuses the fn-pointer coercion,
//! so those three are pinned by a typed call site and the file says so at the pin rather than
//! pretending the mechanism is uniform.
//!
//! Type-level pins follow for the layouts that other code depends on: [`ParamSet`] must stay `Copy`
//! and a fixed size to travel through the kernel's lock-free ring, and [`Factory`] must stay one word
//! so storing one cannot allocate.
//!
//! # What this does not catch, stated rather than implied
//!
//! Pinning signatures detects *changes*. It does not detect *additions* or *removals* of public
//! items: adding a new public function compiles fine, and deleting one fails only where it is pinned
//! here. Catching those needs either `cargo public-api` in CI (a dependency and a toolchain step) or a
//! textual snapshot gate in `tools/`. Neither exists yet; this file is the part that is real today,
//! and it was proven failable by changing a signature on purpose and watching this file fail to
//! compile (build log, WO-007 increment 3).

// type_complexity is allowed ON PURPOSE: this file's whole job is pinning exact higher-ranked
// fn-pointer types, and "factor into a type alias" would hide the signature the pin exists to
// freeze — an alias can be edited without the pin noticing, which defeats the gate.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::type_complexity,
    dead_code
)]

use sparq_kernel::block::BlockContext;
use sparq_module_api::decode::manifest_from;
use sparq_module_api::error::{CodeKind, ValidationError, ValidationReport};
use sparq_module_api::event::{Event, EventBuf, EventKind, EventSink, EVENTS_PER_BLOCK};
use sparq_module_api::manifest::{Manifest, ParamKind, ValidatedManifest};
use sparq_module_api::module::{
    AudioCtx, BlockStatus, CvIn, CvOut, Module, ModuleError, Oversampling, Resources,
    MAX_PORTS_PER_CLASS,
};
use sparq_module_api::params::{ParamBus, ParamSet, ParamSlot, MAX_PARAMS};
use sparq_module_api::port::{
    connect_audio, connect_cross, connect_cv, Adapter, ChannelSet, ChannelSetError, CvInterp,
    CvRange, CvRate, CvReduce, Direction, Multiplicity, Phase, PortType, Verdict,
};
use sparq_module_api::registry::{Factory, RegisterError, Registration, Registry};
use sparq_module_api::toml::{self, Table, TomlError, Value};
use sparq_module_api::{decode, discover, DiscoveryReport, Origin, Source};

// ---------------------------------------------------------------- free functions
const _: fn(&str) -> Result<ValidatedManifest, ValidationReport> = decode;
const _: fn([Source; 0]) -> DiscoveryReport = discover;
const _: fn(&str) -> Result<Table, TomlError> = toml::parse;
const _: fn(ChannelSet, ChannelSet, Phase) -> Verdict = connect_audio;
const _: fn(CvRange, CvRange, Phase) -> Verdict = connect_cv;
const _: fn(PortType, PortType, Phase) -> Verdict = connect_cross;
const _: fn(&Table, &mut ValidationReport) -> Manifest = manifest_from;

// ---------------------------------------------------------------- port vocabulary
const _: fn(PortType) -> &'static str = PortType::as_str;
const _: fn(&str) -> Option<PortType> = PortType::parse;
const _: fn(PortType) -> bool = PortType::audio_thread;
const _: fn(&str) -> Result<ChannelSet, ChannelSetError> = ChannelSet::parse;
const _: fn(ChannelSet) -> Option<usize> = ChannelSet::channels;
const _: fn(ChannelSet) -> bool = ChannelSet::is_spatial;
const _: fn(&str) -> Option<CvRate> = CvRate::parse;
const _: fn(&str) -> Option<CvRange> = CvRange::parse;
const _: fn(&str) -> Option<CvReduce> = CvReduce::parse;
const _: fn(CvReduce) -> &'static str = CvReduce::as_str;
const _: fn(CvReduce, &[f32]) -> f32 = CvReduce::apply;
const _: fn(&str) -> Option<CvInterp> = CvInterp::parse;
const _: fn(CvInterp) -> &'static str = CvInterp::as_str;
// MOVED ON PURPOSE (WO-008 increment 6): `spline` is performed now, and its parabola reads a
// three-knot history and clamps to the wire's declared range — the definition lives in
// `CvInterp::expand`'s doc, the drift gate for the table's copy is tests/compat_matrix.rs.
const _: fn(CvInterp, f32, f32, f32, CvRange, &mut [f32]) = CvInterp::expand;
const _: fn(CvRange, f64) -> f64 = CvRange::clamp_f64;
const _: fn(&str) -> Option<Direction> = Direction::parse;
const _: fn(&str) -> Option<Multiplicity> = Multiplicity::parse;
const _: fn(Adapter) -> &'static str = Adapter::module_id;
const _: fn(Adapter) -> Phase = Adapter::first_phase;
const _: fn(Adapter, Phase) -> bool = Adapter::exists_at;

// ---------------------------------------------------------------- manifest + errors
const _: fn(&Manifest) -> Result<ValidatedManifest, ValidationReport> = Manifest::validate;
const _: fn(&Manifest) -> ValidationReport = Manifest::report;
const _: fn(&ValidatedManifest) -> &Manifest = ValidatedManifest::manifest;
const _: fn(&ValidatedManifest) -> &str = ValidatedManifest::id;
const _: fn(&ValidatedManifest) -> u32 = ValidatedManifest::latency_samples;
const _: fn(&ValidatedManifest) -> bool = ValidatedManifest::latency_is_parametric;
const _: fn(&ValidatedManifest, u32) -> Option<ValidationError> =
    ValidatedManifest::check_declared_latency;
const _: fn(ValidatedManifest) -> Manifest = ValidatedManifest::into_manifest;
const _: fn(&str) -> Option<ParamKind> = ParamKind::parse;
const _: fn(CodeKind) -> &'static str = CodeKind::prefix;
const _: fn(CodeKind) -> bool = CodeKind::is_derived;
const _: fn(CodeKind, &'static str, &'static str) -> ValidationError = ValidationError::new;
const _: fn(&ValidationError) -> String = ValidationError::code;
const _: fn() -> ValidationReport = ValidationReport::new;
const _: fn(&mut ValidationReport, ValidationError) = ValidationReport::push;
const _: fn(&ValidationReport) -> bool = ValidationReport::is_empty;
const _: fn(&ValidationReport) -> usize = ValidationReport::len;
const _: fn(&ValidationReport, CodeKind) -> bool = ValidationReport::has;
const _: fn(&ValidationReport, &str) -> bool = ValidationReport::has_path;
const _: for<'a, 'b> fn(&'a ValidationReport, &'b str) -> Option<&'a ValidationError> =
    ValidationReport::first_at;
const _: fn(&DiscoveryReport) -> Vec<sparq_module_api::discovery::Shadowed> =
    DiscoveryReport::shadowed;
const _: fn(ValidationError, String) -> ValidationError = ValidationError::with_found;
const _: fn(ValidationError, String) -> ValidationError = ValidationError::with_allowed;
const _: fn(&ValidationReport) -> Vec<CodeKind> = ValidationReport::kinds;
const _: fn(ValidationReport) -> Vec<ValidationError> = ValidationReport::into_errors;

// ---------------------------------------------------------------- params
const _: fn() -> ParamSet = ParamSet::zeroed;
const _: fn(u64, &[f32]) -> Option<ParamSet> = ParamSet::new;
const _: fn(ParamSet) -> u64 = ParamSet::version;
const _: fn(ParamSet, usize) -> Option<f32> = ParamSet::get;
const _: fn(ParamSet, usize) -> f32 = ParamSet::at;
const _: fn(&ParamSet) -> &[f32; MAX_PARAMS] = ParamSet::values;
const _: fn(usize) -> (ParamBus, ParamSlot) = ParamBus::new;
const _: fn() -> (ParamBus, ParamSlot) = ParamBus::paired;
const _: fn(&ParamBus, ParamSet) -> bool = ParamBus::publish;
const _: fn(&ParamBus) -> u64 = ParamBus::refusals;
const _: fn(&ParamBus) -> u64 = ParamBus::drops;
const _: fn(&ParamBus) -> usize = ParamBus::pending;
const _: fn(&mut ParamSlot) -> bool = ParamSlot::begin_block;
const _: fn(&ParamSlot) -> ParamSet = ParamSlot::current;
const _: fn(&ParamSlot) -> usize = ParamSlot::pending;

// ---------------------------------------------------------------- module surface
const _: fn(&BlockContext) -> Resources = Resources::from_block;
const _: fn(&str) -> Option<Oversampling> = Oversampling::parse;
const _: fn(Oversampling) -> usize = Oversampling::factor;

// ---------------------------------------------------------------- event payloads (contract v1)
const _: fn(&str) -> Option<EventKind> = EventKind::parse;
const _: fn(EventKind) -> &'static str = EventKind::as_str;
const _: fn(u32, f32) -> Event = Event::trigger;
const _: fn(u32, f32) -> Event = Event::gate;
const _: fn(u32) -> Event = Event::clock;
const _: fn() -> EventBuf = EventBuf::new;
const _: fn(&mut EventBuf) = EventBuf::clear;
const _: fn(&EventBuf) -> &[Event] = EventBuf::as_slice;
const _: fn(&EventBuf) -> usize = EventBuf::len;
const _: fn(&EventBuf) -> bool = EventBuf::is_empty;
const _: fn(&EventBuf) -> usize = EventBuf::capacity;
const _: fn(&EventBuf) -> u64 = EventBuf::dropped;
const _: fn(&EventBuf) -> u64 = EventBuf::dropped_total;
// `EventSink`'s methods are lifetime-generic on the receiver, so they get the typed-call-site
// treatment the header describes rather than fn-pointer coercions rustc refuses.
const _: () = {
    fn pin_sink_push<'a, 'b>(s: &'a mut EventSink<'b>, e: Event) -> bool {
        let pushed: bool = s.push(e);
        pushed
    }
    let _: for<'a, 'b> fn(&'a mut EventSink<'b>, Event) -> bool = pin_sink_push;
    fn pin_sink_len<'a, 'b>(s: &'a EventSink<'b>) -> usize {
        let n: usize = s.len();
        n
    }
    let _: for<'a, 'b> fn(&'a EventSink<'b>) -> usize = pin_sink_len;
    fn pin_sink_capacity<'a, 'b>(s: &'a EventSink<'b>) -> usize {
        let n: usize = s.capacity();
        n
    }
    let _: for<'a, 'b> fn(&'a EventSink<'b>) -> usize = pin_sink_capacity;
};
const _: () = assert!(EVENTS_PER_BLOCK == 64);

// `AudioCtx` and its port views are generic over a lifetime, and rustc will not coerce one of
// their methods to a higher-ranked fn pointer ("one type is more general than the other"). These
// are pinned by typed call sites instead: the annotations fail to compile if a return type
// changes, if a method's receiver changes, or if it disappears. Weaker than a fn-pointer pin in
// one respect — it does not pin the receiver's lifetime — and recorded as such rather than left
// looking equivalent. **Contract v1 (WO-008 increment 4) changed this surface deliberately**: the
// v0 `input`/`output` fields became the `input()`/`output()` first-port views of a multi-port
// context, and the cv/event accessors are new. This file failing to compile on that edit is the
// gate doing its job; the pins below are the reviewed v1 shape.
const _: () = {
    fn pin_audio_ctx<'b>(ctx: &AudioCtx<'b>) -> (usize, bool, f32, &'b [f32]) {
        let frames: usize = ctx.frames();
        let has_input: bool = ctx.has_input();
        let param: f32 = ctx.param(0);
        let input: &'b [f32] = ctx.input();
        (frames, has_input, param, input)
    }
    let _: for<'b> fn(&AudioCtx<'b>) -> (usize, bool, f32, &'b [f32]) = pin_audio_ctx;

    fn pin_audio_ports<'b, 'c>(
        ctx: &'b mut AudioCtx<'c>,
    ) -> (Option<&'c [f32]>, Option<&'b mut [f32]>) {
        let inp: Option<&'c [f32]> = ctx.audio_in(0);
        let out: Option<&'b mut [f32]> = ctx.audio_out(0);
        (inp, out)
    }
    let _: for<'b, 'c> fn(&'b mut AudioCtx<'c>) -> (Option<&'c [f32]>, Option<&'b mut [f32]>) =
        pin_audio_ports;

    fn pin_cv_in<'b>(ctx: &AudioCtx<'b>) -> Option<CvIn<'b>> {
        let cv: Option<CvIn<'b>> = ctx.cv_in(0);
        cv
    }
    let _: for<'b> fn(&AudioCtx<'b>) -> Option<CvIn<'b>> = pin_cv_in;

    fn pin_cv_out<'b, 'c>(ctx: &'b mut AudioCtx<'c>) -> Option<CvOut<'b>> {
        let cv: Option<CvOut<'b>> = ctx.cv_out(0);
        cv
    }
    let _: for<'b, 'c> fn(&'b mut AudioCtx<'c>) -> Option<CvOut<'b>> = pin_cv_out;

    fn pin_events_in<'b>(ctx: &AudioCtx<'b>) -> Option<&'b [Event]> {
        let evs: Option<&'b [Event]> = ctx.events_in(0);
        evs
    }
    let _: for<'b> fn(&AudioCtx<'b>) -> Option<&'b [Event]> = pin_events_in;

    fn pin_event_out<'b, 'c>(ctx: &'b mut AudioCtx<'c>) -> Option<EventSink<'b>> {
        let sink: Option<EventSink<'b>> = ctx.event_out(0);
        sink
    }
    let _: for<'b, 'c> fn(&'b mut AudioCtx<'c>) -> Option<EventSink<'b>> = pin_event_out;

    // The take_* variants move views out with the CONTEXT's lifetime — the multi-port pattern.
    fn pin_take_audio_out<'b, 'c>(ctx: &'b mut AudioCtx<'c>) -> Option<&'c mut [f32]> {
        let out: Option<&'c mut [f32]> = ctx.take_audio_out(0);
        out
    }
    let _: for<'b, 'c> fn(&'b mut AudioCtx<'c>) -> Option<&'c mut [f32]> = pin_take_audio_out;

    fn pin_take_cv_out<'b, 'c>(ctx: &'b mut AudioCtx<'c>) -> Option<CvOut<'c>> {
        let cv: Option<CvOut<'c>> = ctx.take_cv_out(0);
        cv
    }
    let _: for<'b, 'c> fn(&'b mut AudioCtx<'c>) -> Option<CvOut<'c>> = pin_take_cv_out;

    fn pin_take_event_out<'b, 'c>(ctx: &'b mut AudioCtx<'c>) -> Option<EventSink<'c>> {
        let sink: Option<EventSink<'c>> = ctx.take_event_out(0);
        sink
    }
    let _: for<'b, 'c> fn(&'b mut AudioCtx<'c>) -> Option<EventSink<'c>> = pin_take_event_out;

    fn pin_v0_output<'b, 'c>(ctx: &'b mut AudioCtx<'c>) -> &'b mut [f32] {
        let out: &'b mut [f32] = ctx.output();
        out
    }
    let _: for<'b, 'c> fn(&'b mut AudioCtx<'c>) -> &'b mut [f32] = pin_v0_output;
};

// The cv view enums, pinned by typed call sites (CvIn is Copy and lifetime-generic; CvOut borrows).
const _: () = {
    fn pin_cv_in_views<'b>(cv: CvIn<'b>) -> (bool, Option<f32>, Option<&'b [f32]>, f32) {
        let connected: bool = cv.is_connected();
        let block: Option<f32> = cv.block();
        let audio: Option<&'b [f32]> = cv.audio();
        let at: f32 = cv.at(0);
        (connected, block, audio, at)
    }
    let _: for<'b> fn(CvIn<'b>) -> (bool, Option<f32>, Option<&'b [f32]>, f32) = pin_cv_in_views;

    fn pin_cv_out_views<'b>(mut cv: CvOut<'b>) {
        cv.set(0.5);
        let _block: Option<&f32> = cv.as_block();
        let _audio: Option<&mut [f32]> = cv.as_audio();
    }
    let _: for<'b> fn(CvOut<'b>) = pin_cv_out_views;
};

// Constructors and builders: the host-side assembly surface of contract v1.
const _: fn(&'static BlockContext, &'static ParamSet) -> AudioCtx<'static> = AudioCtx::new;
const _: fn(
    &'static BlockContext,
    &'static ParamSet,
    &'static [f32],
    &'static mut [f32],
) -> AudioCtx<'static> = AudioCtx::single;
const _: fn(AudioCtx<'static>, &'static [f32]) -> AudioCtx<'static> = AudioCtx::with_audio_in;
const _: fn(AudioCtx<'static>, &'static mut [f32]) -> AudioCtx<'static> = AudioCtx::with_audio_out;
const _: fn(AudioCtx<'static>, CvIn<'static>) -> AudioCtx<'static> = AudioCtx::with_cv_in;
const _: fn(AudioCtx<'static>, CvOut<'static>) -> AudioCtx<'static> = AudioCtx::with_cv_out;
const _: fn(AudioCtx<'static>, &'static [Event]) -> AudioCtx<'static> = AudioCtx::with_events_in;
const _: fn(AudioCtx<'static>, &'static mut EventBuf) -> AudioCtx<'static> =
    AudioCtx::with_event_out;
const _: () = {
    fn pin_sink_new<'a>(b: &'a mut EventBuf) -> EventSink<'a> {
        let sink: EventSink<'a> = EventSink::new(b);
        sink
    }
    let _: for<'a> fn(&'a mut EventBuf) -> EventSink<'a> = pin_sink_new;
};

// ---------------------------------------------------------------- registry + discovery
const _: fn() -> Registry = Registry::new;
const _: fn(&mut Registry, &str, Factory) -> Result<(), RegisterError> = Registry::register;
const _: fn(&mut Registry, ValidatedManifest, Factory) -> Result<(), RegisterError> =
    Registry::register_validated;
const _: fn(&Registry) -> usize = Registry::len;
const _: fn(&Registry) -> bool = Registry::is_empty;
const _: for<'a, 'b> fn(&'a Registry, &'b str) -> Option<&'a Registration> = Registry::get;
const _: for<'a, 'b, 'c> fn(&'a Registry, &'b str, &'c str) -> Option<&'a Registration> =
    Registry::get_pinned;
const _: fn(&Registry, &str) -> Option<Box<dyn Module>> = Registry::create;
const _: fn(&Registry) -> Vec<&str> = Registry::ids;
const _: fn(&Registration) -> &ValidatedManifest = Registration::manifest;
const _: fn(&Registration) -> Box<dyn Module> = Registration::create;
const _: fn(&Registration) -> &str = Registration::id;
const _: fn(&Registration) -> &str = Registration::version;
const _: fn(Origin) -> u8 = Origin::precedence;
const _: fn(Origin, String, String) -> Source = Source::new;
const _: fn(&DiscoveryReport) -> bool = DiscoveryReport::is_empty;
const _: fn(&DiscoveryReport) -> usize = DiscoveryReport::len;
const _: fn(&DiscoveryReport) -> Vec<&str> = DiscoveryReport::ids;
const _: for<'a, 'b> fn(
    &'a DiscoveryReport,
    &'b str,
) -> Option<&'a sparq_module_api::discovery::Loaded> = DiscoveryReport::get;
const _: fn(&DiscoveryReport) -> String = DiscoveryReport::render;

// ---------------------------------------------------------------- toml surface
const _: for<'a, 'b> fn(&'a Table, &'b str) -> Option<&'a Value> = Table::get;
const _: fn(&Table, &str) -> Option<usize> = Table::line_of;
const _: fn(&Table) -> usize = Table::len;
const _: fn(&Table) -> bool = Table::is_empty;
const _: for<'a, 'b> fn(&'a Table, &'b str) -> Option<Vec<&'a Table>> = Table::tables;
const _: fn(&Value) -> Option<&str> = Value::as_str;
const _: fn(&Value) -> Option<i64> = Value::as_i64;
const _: fn(&Value) -> Option<u32> = Value::as_u32;
const _: fn(&Value) -> Option<f64> = Value::as_f64;
const _: fn(&Value) -> Option<bool> = Value::as_bool;
const _: fn(&Value) -> Option<&[Value]> = Value::as_array;
const _: fn(&Value) -> Option<&Table> = Value::as_table;
const _: fn(&Value) -> &'static str = Value::type_name;

// ---------------------------------------------------------------- layout pins
// ParamSet crosses to the audio thread through a lock-free ring slot, so its size and Copy-ness are
// part of the contract, not an implementation detail.
const _: () = assert!(std::mem::size_of::<ParamSet>() == 8 + 4 * MAX_PARAMS);
const _: () = assert!(std::mem::size_of::<Factory>() == std::mem::size_of::<usize>());
const _: () = assert!(MAX_PARAMS == 32);
// Contract v1's fixed capacities: the context is assembled per node per block on the audio thread,
// and the event storage is a fixed array — both sizes are load-bearing, so both are pinned.
const _: () = assert!(MAX_PORTS_PER_CLASS == 8);
const _: () = assert!(std::mem::size_of::<Event>() <= 64);
const _: () = assert!(std::mem::size_of::<EventBuf>() <= 64 * std::mem::size_of::<Event>() + 32);
const _: () = assert!(
    std::mem::size_of::<AudioCtx<'static>>()
        <= 24 * std::mem::size_of::<usize>() * MAX_PORTS_PER_CLASS
);

fn assert_copy<T: Copy>() {}
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn the_param_snapshot_is_copy_and_thread_crossable() {
    // If this stops compiling, the audio thread's parameter path has changed shape.
    assert_copy::<ParamSet>();
    assert_send_sync::<ParamBus>();
    assert_send_sync::<ParamSlot>();
    assert_send_sync::<Registry>();
}

#[test]
fn the_module_trait_is_object_safe_so_the_executor_can_hold_modules_in_a_vec() {
    // ADR-009 executor decision 7: block-level dispatch may be a trait object. This is the compile
    // proof, and it is a *test* rather than a const so that a regression is reported as a test
    // failure rather than only as a build error.
    fn takes_dyn(_: &dyn Module) {}
    fn takes_boxed(_: Box<dyn Module>) {}
    fn make<T: Module + 'static>(_: T) -> Box<dyn Module> {
        unimplemented!("this fn is never called; it exists to pin the bound")
    }
    let _ = takes_dyn as fn(&dyn Module);
    let _ = takes_boxed as fn(Box<dyn Module>);
    let _ = make::<ProbeModule> as fn(ProbeModule) -> Box<dyn Module>;
}

#[test]
fn a_module_trait_object_is_send_because_the_cross_thread_engine_moves_it() {
    // WO-008 increment 5's contract change, pinned: a module is built on the control thread,
    // rendered on the audio thread and retired back on the control thread, so `Module: Send` is
    // part of the contract (ADR-009 decision 3's hot swap). `Sync` is deliberately NOT pinned:
    // a module is never touched by two threads at once, and the executor's `&mut` proves it.
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn Module>>();
    assert_send::<ProbeModule>();
}

/// The smallest possible module, so the trait bound above has something to instantiate.
struct ProbeModule;

impl Module for ProbeModule {
    fn id(&self) -> &str {
        "sparq/test/probe"
    }
    fn configure(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, _ctx: &mut AudioCtx<'_>) -> BlockStatus {
        BlockStatus::Ok
    }
    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[test]
fn the_layout_pins_are_the_ones_the_audio_path_depends_on() {
    // The `const` pins above are compile-time; these assert the same facts at runtime, so a change
    // shows up in a test report rather than only in a build log.
    assert_eq!(std::mem::size_of::<ParamSet>(), 8 + 4 * MAX_PARAMS);
    assert_eq!(std::mem::size_of::<Factory>(), std::mem::size_of::<usize>());
    assert_eq!(BlockStatus::default(), BlockStatus::Ok);
    fn assert_copy<T: Copy>() {}
    assert_copy::<ParamSet>();
}
