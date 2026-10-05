//! The `gpu_class` ceiling table — contract v1.1's freeze debt #1, CLOSED as data (WO-018).
//!
//! The v1.1 freeze (`docs/api/instrument-wit/README.md` §5) froze the *spellings* of
//! `resources.gpu_class` and left the numbers to the host: "gpu_class → vertex/instance/cell
//! ceiling table (renderer side)". This is that table. It is the budget a `scene` or display-list
//! renderer schedules against (MODULE-BUILD-GUIDE §5: over budget the host degrades LOD, and at
//! Full-LOD refusal the display shows *words*, not garbage).
//!
//! # Anchors — the numbers are a rationale, not a decoration
//!
//! * `light` is exactly **one 256×256 heightfield** — the fm-terrain walkthrough case the guide
//!   proves the contract with — and **one 128×128 heat-cell grid**: 65 536 vertices, 16 384
//!   cells, 4 096 instances for the glyph/point-sprite draws that annotate them.
//! * Each class step multiplies **every** column by 4, so the degradation ladder has even rungs
//!   and "one class up" always means the same thing whatever the column.
//! * `none` is the zero row: a module that emits no scene data at all — the common case for
//!   backbone-style panels. The zero row cannot be honoured by a `scene` display, so the
//!   validator refuses that combination statically (`validate`'s scene⇒gpu_class rule) rather
//!   than letting the first draw discover it.
//!
//! # Movement rule
//!
//! Numbers move **UP** only, and only with a recorded re-baselining (renderer measurements on the
//! stage machine, logged in the build record like every golden change). They never drift down
//! silently: an instrument authored against `medium` must keep fitting in `medium`. The spellings
//! are the frozen part — `sparq_module_api::manifest::GPU_CLASSES`, drift-pinned against
//! `docs/api/manifest-fields.toml` and value-pinned in `tests/api_snapshot.rs`.

/// The scene-data ceilings for one `gpu_class`, per display frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuCeilings {
    /// Total vertices across `points`/`lines`/`mesh`/`heightfield` draws. A heightfield grid of
    /// w×h costs w×h vertices (its indices are the grid's own).
    pub max_vertices: u32,
    /// Instanced draws — repeated glyph runs, point sprites, per-cell quads the renderer batches.
    pub max_instances: u32,
    /// `heat_cells` entries. Gridded magnitude/signed data is the sonogram/rig-map case, and it
    /// is budgeted separately from vertices because its renderer path is a texture write, not a
    /// vertex pull.
    pub max_heat_cells: u32,
}

/// The table, in `GPU_CLASSES` order — `none` first, `very_heavy` last, ~4× per step (see the
/// module header for the anchors and the movement rule).
pub const TABLE: [(&str, GpuCeilings); 5] = [
    ("none", GpuCeilings { max_vertices: 0, max_instances: 0, max_heat_cells: 0 }),
    ("light", GpuCeilings { max_vertices: 65_536, max_instances: 4_096, max_heat_cells: 16_384 }),
    (
        "medium",
        GpuCeilings { max_vertices: 262_144, max_instances: 16_384, max_heat_cells: 65_536 },
    ),
    (
        "heavy",
        GpuCeilings { max_vertices: 1_048_576, max_instances: 65_536, max_heat_cells: 262_144 },
    ),
    (
        "very_heavy",
        GpuCeilings { max_vertices: 4_194_304, max_instances: 262_144, max_heat_cells: 1_048_576 },
    ),
];

/// The ceilings for a declared class spelling.
///
/// `None` when the spelling is outside the frozen vocabulary. The decoder's domain check
/// (`decode.rs::check_gpu_class`) has already refused such a manifest at discovery; this stays
/// total anyway, because the host side of a contract never trusts that the guest side ran.
#[must_use]
pub fn ceilings_for(class: &str) -> Option<GpuCeilings> {
    TABLE.iter().find(|(name, _)| *name == class).map(|(_, c)| *c)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn light_anchors_the_table() {
        // The recorded anchor (WO-018 judgement call 1, operator-reviewable): `light` is exactly
        // one 256×256 heightfield — the fm-terrain case — and one 128×128 heat-cell grid. If
        // this assertion ever fails, the table moved without its rationale, which is the
        // re-baselining rule forbidding.
        let light = ceilings_for("light").unwrap();
        assert_eq!(light.max_vertices, 256 * 256, "one 256×256 heightfield");
        assert_eq!(light.max_heat_cells, 128 * 128, "one 128×128 heat-cell grid");
        assert_eq!(light.max_instances, 4_096);

        // And `none` is the zero row that cannot be honoured by a scene — the reason the
        // validator's scene⇒gpu_class rule exists.
        let none = ceilings_for("none").unwrap();
        assert_eq!(none, GpuCeilings { max_vertices: 0, max_instances: 0, max_heat_cells: 0 });
    }

    #[test]
    fn the_table_covers_the_frozen_spellings_and_steps_by_four() {
        // The spellings are the contract crate's frozen vocabulary: every class resolves, in the
        // contract's order, and each step is exactly 4× in every column — the even-rungs property
        // the degradation ladder schedules against.
        for (i, class) in sparq_module_api::manifest::GPU_CLASSES.iter().enumerate() {
            let (name, c) = TABLE[i];
            assert_eq!(name, *class, "TABLE drifted from GPU_CLASSES at {i}");
            assert!(ceilings_for(class).is_some(), "{class} has no row");
            // The 4x step starts at `light`: the zero row has nothing to multiply, and its
            // distance to `light` IS the anchor test above.
            if i > 1 {
                let prev = TABLE[i - 1].1;
                assert_eq!(c.max_vertices, prev.max_vertices * 4, "{class} vertices");
                assert_eq!(c.max_instances, prev.max_instances * 4, "{class} instances");
                assert_eq!(c.max_heat_cells, prev.max_heat_cells * 4, "{class} cells");
            }
        }
        // An unknown spelling gets nothing — the host side never trusts that the decoder ran.
        assert_eq!(ceilings_for("trivial"), None, "cpu_class spellings are not gpu_class ones");
        assert_eq!(ceilings_for("ultra"), None);
    }
}
