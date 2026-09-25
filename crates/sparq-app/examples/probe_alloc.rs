//! Standalone allocation probe for the audio path.
//!
//! `cargo run -p sparq-app --example probe_alloc`
//!
//! This exists because the answer to "does the audio path allocate?" must be measurable outside the
//! test harness, where harness lazy-init can pollute a cold measurement (see the comment in
//! `crates/sparq-audio/tests/rt_discipline.rs`). It installs the counting allocator for the whole
//! binary and reports allocations per window of 1 000 blocks.

#![allow(clippy::print_stdout)] // it is a probe: printing is the point

use sparq_audio::graph::{GraphConfig, Wo005Graph};
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::block::BlockContext;
use sparq_kernel::device::StreamConfig;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

fn main() {
    let stream = StreamConfig {
        sample_rate: 96_000,
        block_frames: 64,
        inputs: 0,
        outputs: 2,
        exclusive: false,
    };
    let mut graph = Wo005Graph::new(GraphConfig { stream, ..GraphConfig::default() });
    let mut buf = vec![0.0f32; stream.block_frames * stream.outputs];
    let ctx = BlockContext::offline(stream.sample_rate, stream.block_frames, stream.outputs);

    println!(
        "sparq allocation probe · {} Hz · block {} · {} ch",
        stream.sample_rate, stream.block_frames, stream.outputs
    );
    println!("  audit live: {}", sparq_kernel::alloc::audit_is_live());
    for round in 0..5 {
        let base = start_counting();
        for _ in 0..1000 {
            graph.process(&ctx, &mut buf);
            std::hint::black_box(&buf);
        }
        let made = allocation_count().saturating_sub(base);
        stop_counting();
        println!("  window {round}: {made} allocations / 1000 blocks");
        assert_eq!(made, 0, "audio path allocated");
    }
    println!("  result: 0 allocations across 5000 blocks");
}
