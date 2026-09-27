//! WO-009's integration acceptance, at the level that matters: the TRANSPORT-driven drum demo
//! must hash to the same golden as the app-side block-counter schedule batch 3 pinned
//! (`f2303f13aa0cf299`) — two independent trigger mechanisms producing one bit pattern is the
//! strongest available statement that the clocks, the event door and the executor agree about
//! where a beat lives. Plus the WO's criterion 4 against a real graph: the same play/stop/play
//! script twice renders identically, and the stop freezes the MUSIC, not the stream.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::ExecConfig;
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::{drum_demo_patch, register_builtins};
use sparq_module_api::registry::Registry;
use sparq_music::transport::{BlockEvents, Transport};

const RATE: u32 = 48_000;
const FRAMES: usize = 64;
/// The batch-3 golden — the app-side `is_kick_block` schedule's render. Pinned there, asserted
/// here: if the transport's beat grid and the block counter's ever disagree by one sample, this
/// fails with the two hashes side by side.
const APP_SCHEDULED_GOLDEN: &str = "f2303f13aa0cf299";

/// Render the drum demo with the transport driving; optionally stop/resume at block numbers.
/// Returns the render hash and the absolute render frames where beat triggers (channel 0) were
/// injected.
fn run(blocks: usize, stop_resume: Option<(usize, usize)>) -> (String, Vec<u64>) {
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();
    let cfg = ExecConfig::new(RATE, FRAMES, 2);
    let mut demo = drum_demo_patch(&reg, cfg).unwrap();
    let mut tr = Transport::new(RATE, 120.0, 960);
    tr.play();
    let mut events = BlockEvents::new();
    let mut out = vec![0.0f32; FRAMES * 2];
    let mut samples: Vec<f32> = Vec::with_capacity(blocks * FRAMES * 2);
    let mut kicks: Vec<u64> = Vec::new();
    for b in 0..blocks {
        if let Some((s, r)) = stop_resume {
            if b == s {
                tr.stop();
            }
            if b == r {
                tr.play();
            }
        }
        let tick0 = tr.abs_tick().round().max(0.0) as u64;
        demo.executor.set_musical_position(Some((tick0, 960)));
        tr.advance_block(FRAMES, &mut events);
        for e in events.as_slice() {
            demo.executor.push_host_event(demo.membrane, 0, *e).unwrap();
            if e.channel == 0 {
                kicks.push(b as u64 * FRAMES as u64 + u64::from(e.sample));
            }
        }
        demo.executor.render_block(demo.mixer, &mut out).unwrap();
        samples.extend_from_slice(&out);
    }
    (hex64(fnv1a64_f32(&samples)), kicks)
}

#[test]
fn the_transport_driven_drum_demo_hashes_to_the_app_scheduled_golden() {
    let (hash, kicks) = run(1500, None);
    // 2 s at 120 BPM: beats at render frames 0, 24000, 48000, 72000 — the map's own samples.
    assert_eq!(kicks, vec![0, 24_000, 48_000, 72_000], "the beat grid is the map's");
    assert_eq!(
        hash, APP_SCHEDULED_GOLDEN,
        "the transport-driven render diverged from the block-counter golden — the two mechanisms \
         disagree about where a beat lives, and one of them is lying"
    );
}

#[test]
fn stop_freezes_the_music_not_the_stream_and_the_script_is_repeatable() {
    // Stop at block 500 (transport position 32000), resume at 900. The render stream keeps
    // running — the paused stretch is silence — and the music resumes exactly where it froze:
    // beat 3 (tick 2880 = sample 48000) needs 16000 more transport samples after the resume,
    // which is 250 playing blocks: block 1150, render frame 73600. Beat 4 (72000 transport)
    // would need block 1525 — past the 1500-block render, so three kicks total, at arithmetic
    // positions, not vibes.
    let (h1, k1) = run(1500, Some((500, 900)));
    let (h2, k2) = run(1500, Some((500, 900)));
    assert_eq!(k1, vec![0, 24_000, 73_600], "the paused stretch moved the music, not the stream");
    assert_eq!(k1, k2, "the same script twice fires the same triggers at the same samples");
    assert_eq!(h1, h2, "the same script twice renders identically — WO-009 criterion 4");
    assert_ne!(
        h1, APP_SCHEDULED_GOLDEN,
        "a 200-block pause must AUDIBLY change the render, or the stop is not real"
    );
}
