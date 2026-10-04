//! `noop-instrument` — the compile-verified reference skeleton for the guest SDK.
//!
//! Not a musical instrument: the smallest honest component that satisfies the contract —
//! silence in the exact negotiated shapes, an empty (at-rest) display, state refused in words.
//! It exists so `cargo build --target wasm32-wasip1` in this directory is a machine check
//! that the SDK, the WIT contract and the component toolchain agree — the guest-side half of
//! WO-017's round-trip test (the host-side half lands with WO-018's loader).
//!
//! Build both faces:
//! ```sh
//! cargo build -p noop-instrument --target wasm32-wasip1   # the component face
//! cargo test  -p noop-instrument                          # native unit tests of the logic
//! ```

use sparq_module_guest as sparq;
use sparq::{AudioBuf, BlockInput, BlockOutput, BlockStatus, CvInBuf, CvOutBuf, InstrumentModule, ModuleError, Resources};

/// The instrument: no state worth keeping, which `configure`/`save_state`'s defaults say
/// honestly rather than silently.
#[derive(Default)]
pub struct Noop;

impl InstrumentModule for Noop {
    fn id(&self) -> String {
        // Must equal identity.id in the manifest it ships with. `example/` namespace: this is a
        // test artefact, not a first-party module (the `sparq/` namespace is reserved).
        "example/syn/noop".to_string()
    }

    fn prepare(&mut self, resources: Resources) -> Result<(), ModuleError> {
        // Nothing to allocate — but a real instrument sizes every buffer HERE, never in process.
        if resources.block_frames == 0 {
            return Err(ModuleError::Resources("block-frames must be > 0".into()));
        }
        Ok(())
    }

    fn process(&mut self, input: BlockInput) -> BlockOutput {
        // Silence in the EXACT negotiated shapes: one entry per output port, channel counts
        // from prepare's resources, per-frame lists exactly `frames` long. A shape mismatch is
        // treated as `failed` by the host — shape discipline is part of the contract, so the
        // noop demonstrates it by mirroring its inputs.
        let frames = usize::try_from(input.frames).unwrap_or(0);
        BlockOutput {
            status: BlockStatus::Silenced, // deliberate silence is a statement, not a surprise
            audio_out: input
                .audio_in
                .iter()
                .map(|buf| AudioBuf {
                    channels: buf.channels,
                    connected: true,
                    interleaved: vec![0.0; usize::try_from(buf.channels).unwrap_or(0) * frames],
                })
                .collect(),
            cv_out: input
                .cv_in
                .iter()
                .map(|cv| match cv {
                    CvInBuf::PerFrame(values) => CvOutBuf::PerFrame(vec![0.0; values.len()]),
                    CvInBuf::PerBlock(_) | CvInBuf::Unconnected => CvOutBuf::PerBlock(0.0),
                })
                .collect(),
            events_out: Vec::new(),
            data_out: Vec::new(),
        }
    }

    // activate/deactivate/configure/save_state/message/draw: the SDK defaults are this
    // instrument's honest answers — no state ("this instrument keeps no state"), no messages,
    // and an at-rest display that shows nothing rather than garbage.
}

sparq::sparq_instrument!(Noop);

#[cfg(test)]
mod tests {
    use super::*;
    use sparq::{Oversampling, Semver};

    fn resources() -> Resources {
        Resources {
            sample_rate: 48_000,
            block_frames: 64,
            audio_in_channels: vec![1],
            audio_out_channels: vec![1],
            oversampling: Oversampling::None,
            voices: 0,
            arena_bytes: 0,
            fuel_per_block: 1_000_000,
            token_bundle: Semver { major: 1, minor: 1, patch: 0 },
        }
    }

    #[test]
    fn prepare_refuses_a_zero_block_in_words() {
        let mut noop = Noop;
        assert!(noop.prepare(resources()).is_ok());
        let mut broken = resources();
        broken.block_frames = 0;
        match noop.prepare(broken) {
            Err(ModuleError::Resources(why)) => assert!(why.contains("block-frames")),
            other => panic!("expected a resources refusal with words, got {other:?}"),
        }
    }

    #[test]
    fn process_returns_silence_in_the_negotiated_shape() {
        let mut noop = Noop;
        noop.prepare(resources()).expect("prepare must succeed");
        let input = BlockInput {
            block_id: 0,
            frames: 64,
            t_sample: 0,
            tick: 0,
            ppqn: 960,
            params: sparq::ParamSet { version: 0, values: Vec::new() },
            audio_in: vec![AudioBuf { channels: 2, connected: true, interleaved: vec![0.0; 128] }],
            cv_in: vec![CvInBuf::PerBlock(0.5)],
            events_in: Vec::new(),
            data_in: Vec::new(),
        };
        let out = noop.process(input);
        assert!(matches!(out.status, BlockStatus::Silenced));
        assert_eq!(out.audio_out.len(), 1, "one entry per output port");
        assert_eq!(out.audio_out[0].interleaved.len(), 128, "channels × frames, exact");
        assert_eq!(out.cv_out.len(), 1);
        assert!(matches!(out.cv_out[0], CvOutBuf::PerBlock(0.0)));
    }
}
