//! The live display plane's host-side seams (WO-020 INC6 S4, plan D17c/D19): the paced guest
//! draw, the frame-skip/bypass watchdog and the live/at-rest band switch.
//!
//! The wasmtime half is DEVICE-FIRST-COMPILE (the §8 memory wall stands — anything pulling
//! wasmtime OOMs the 1 GB sandbox): `ui/instrument_launch.rs` adapts the runtime's display
//! `GuestInstance` onto the [`LiveDisplay`] trait there, and the shell never knows which side
//! of the seam it is on. Everything in THIS file is sandbox-proven: the pacer's cadence is the
//! D19 token, the skip/bypass counter is the plan's own arithmetic (five consecutive), and the
//! band switch is the meters' rule extended — "live where the instance carries it, at rest
//! otherwise", never a frozen lie.
//!
//! The rules, from the plan of record:
//!
//! * **D19 — the display cadence is a token, default 15 Hz, plus an immediate draw on edits.**
//!   A full wall draw measured 12.4 M fuel warm; 60 Hz guest draws would tax the UI thread for
//!   a data wall whose fastest feed is a 5 s cadence. Any param edit, cell swap, resize or LOD
//!   change forces the next frame's draw regardless of the pace — the edit's effect is
//!   immediate, the ambient scroll is paced.
//! * **D17c — the fuel budget is the declared `max_fuel` per call; an overrun skips the frame
//!   and counts; five consecutive bypass the DISPLAY instance** with §3's words in the panel
//!   band — the audio instance is untouched (decision D's isolation: two instances, two
//!   watchdogs, never one mechanism).
//! * **The frame-context's `time_sec` is the shell's animation clock** (legal: a display is
//!   never replayed — D8), so the ticker scrolls and the guest's PAUSE param freezes it.

use std::collections::HashMap;

use sparq_ui::canvas::model::NodeId;
use sparq_ui::displaylist::Item;
use sparq_ui::tokens::LAYOUT_CANVAS_INSTRUMENT_DISPLAY_HZ;

/// The LOD as the display contract spells it — the host-side mirror of the WIT `display.Lod`
/// (the device adapter in `instrument_launch.rs` converts; sandbox mocks read this directly).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayLod {
    /// Full detail.
    Full,
    /// The guest's simplified ladder rung (§5.7).
    Simplified,
    /// The dot rung.
    Dot,
}

/// One paced draw's context (D17c): the band's ACTUAL px — so the live guest re-renders at the
/// size the card really is and text stays native (O-1; S1's resize reaches the guest through
/// this struct) — the camera's LOD, and the shell's animation clock.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayFrame {
    /// The manifest's display id (`wall` for the Observatory — v0 draws `ui.displays[0]`).
    pub display_id: String,
    /// The band's width, logical px (the guest lays out against exactly this).
    pub width_px: u32,
    /// The band's height, logical px.
    pub height_px: u32,
    /// The camera's LOD, in the contract's spelling.
    pub lod: DisplayLod,
    /// The shell's animation clock, seconds — the ticker scrolls on it; the guest's PAUSE
    /// param freezes the motion against it (D8: a display is never replayed).
    pub time_sec: f64,
}

/// A live display instance behind the seam. The device implementation wraps the runtime's
/// display `GuestInstance`: `budget_call` at the declared `max_fuel` per draw, the WIT
/// `surface` through the D13 interchange into the SAME parser the at-rest store uses (one
/// interchange, two producers). Test implementations are scripted.
///
/// `Err` carries §3's watchdog words verbatim (`fuel budget exhausted …`, `epoch deadline …`,
/// or the trap's own sentence) — the pacer counts them; the panel band shows the bypass
/// sentence, never a raw value.
pub trait LiveDisplay {
    /// One draw at the frame's context. The surface is the band's live display list.
    fn draw(&mut self, frame: &DisplayFrame) -> Result<Vec<Item>, String>;
    /// The display id this instance draws.
    fn display_id(&self) -> &str;
}

/// How many CONSECUTIVE overruns bypass the DISPLAY instance (plan D17c, measured not hoped:
/// the audit's injection proof shrinks a budget and watches this fire). The audio side's
/// watchdog keeps its own N in the executor — decision D's isolation.
pub const BYPASS_AFTER: u32 = 5;

/// The §3 bypass sentence (instrument-host.md §3's mapping row: frames dropped and counted,
/// N consecutive → the DISPLAY instance bypassed, the panel shows words, the audio instance
/// untouched). It rides the panel band's word strip — the same slot the provider's sentence
/// and the at-rest sentence use; the bypass OUTRANKS both (a watchdog fact beats a status).
pub const BYPASS_WORDS: &str = "DISPLAY BYPASSED — five consecutive draws overran; the last render stands (the audio instance is untouched)";

#[derive(Debug, Default)]
struct NodePace {
    /// The last draw attempt's clock (a SKIP counts as an attempt — the pace is a floor on
    /// draws, and an overrun must not tight-loop the guest on the next frame).
    last_draw_ms: Option<u64>,
    /// An edit forces the next frame's draw (D19).
    forced: bool,
    /// Consecutive failed draws (the bypass counter).
    consecutive_errs: u32,
    /// The instance is bypassed: no draws, the band shows [`BYPASS_WORDS`] over the at-rest
    /// fallback, until an edit re-arms it (the plan names no other re-arm door; an edit is the
    /// user saying "try again" — recorded as a judgment call in the slice's state card).
    bypassed: bool,
}

/// The display pacer (D19): one gate per instrument node — `due` says whether this frame
/// draws, `record` absorbs the outcome. The cadence is the token; the force is the edit.
#[derive(Debug)]
pub struct DisplayPacer {
    nodes: HashMap<NodeId, NodePace>,
    period_ms: u64,
}

impl Default for DisplayPacer {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayPacer {
    /// A pacer at the D19 token's cadence.
    #[must_use]
    pub fn new() -> Self {
        let hz = (LAYOUT_CANVAS_INSTRUMENT_DISPLAY_HZ as u64).max(1);
        Self { nodes: HashMap::new(), period_ms: 1000 / hz }
    }

    /// The paced period, ms (the token, derived once — 66 ms at 15 Hz).
    #[must_use]
    pub fn period_ms(&self) -> u64 {
        self.period_ms
    }

    /// An edit landed on this node (a param — including a cell swap through the picker — or a
    /// resize): the next frame draws regardless of the pace (D19), and a bypassed instance
    /// re-arms (the retry door).
    pub fn note_edit(&mut self, node: NodeId) {
        let p = self.nodes.entry(node).or_default();
        p.forced = true;
        p.bypassed = false;
        p.consecutive_errs = 0;
    }

    /// The camera's LOD changed: every instance's next frame draws (D19 — the guest's own
    /// ladder rung moves, and a stale-rung surface would be a lie).
    pub fn note_lod_change(&mut self) {
        for p in self.nodes.values_mut() {
            p.forced = true;
        }
    }

    /// Whether `node` draws this frame at `now_ms`. A node never seen draws (the first frame
    /// after a spawn/registration is an edit's immediacy); a bypassed node never draws.
    #[must_use]
    pub fn due(&self, node: NodeId, now_ms: u64) -> bool {
        let Some(p) = self.nodes.get(&node) else { return true };
        if p.bypassed {
            return false;
        }
        p.forced || p.last_draw_ms.map_or(true, |t| now_ms >= t + self.period_ms)
    }

    /// Absorb one draw attempt's outcome. `ok` clears the consecutive count; a failure counts
    /// toward the bypass (five consecutive — [`BYPASS_AFTER`]).
    pub fn record(&mut self, node: NodeId, ok: bool, now_ms: u64) {
        let p = self.nodes.entry(node).or_default();
        p.forced = false;
        p.last_draw_ms = Some(now_ms);
        if ok {
            p.consecutive_errs = 0;
        } else {
            p.consecutive_errs += 1;
            if p.consecutive_errs >= BYPASS_AFTER {
                p.bypassed = true;
            }
        }
    }

    /// Whether the node's DISPLAY instance is bypassed (the painter's word-strip gate).
    #[must_use]
    pub fn bypassed(&self, node: NodeId) -> bool {
        self.nodes.get(&node).is_some_and(|p| p.bypassed)
    }

    /// A node left the canvas: drop its pace state.
    pub fn forget(&mut self, node: NodeId) {
        self.nodes.remove(&node);
    }
}

/// One live render, as the band reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveSurface {
    /// The display list, in the host painter's parsed form (the at-rest store's own shape).
    pub items: Vec<Item>,
    /// The logical width it was drawn at (the band's actual px at draw time — O-1).
    pub w: f32,
    /// The logical height.
    pub h: f32,
    /// The shell clock it was drawn at (diagnostics; the strip's age words could ride it —
    /// v0 does not: a live surface is NOW by definition, the broker's ages are the stream's).
    pub drawn_ms: u64,
}

/// The live-surface store, keyed by module id (the at-rest store's own key — one band, two
/// shelves, and the switch between them is the meters' rule extended: LIVE where the instance
/// carries it, AT REST otherwise, never a frozen lie).
#[derive(Clone, Debug, Default)]
pub struct LiveSurfaces {
    surfaces: HashMap<String, LiveSurface>,
}

impl LiveSurfaces {
    /// A fresh (empty) store — an empty store means "nothing is live", and the painter falls
    /// back to the at-rest shelf everywhere.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert/replace one module's live surface (the driver tick after a successful draw).
    pub fn insert(&mut self, module_id: &str, surface: LiveSurface) {
        self.surfaces.insert(module_id.to_string(), surface);
    }

    /// The live surface for a module, if the instance carries one.
    #[must_use]
    pub fn get(&self, module_id: &str) -> Option<&LiveSurface> {
        self.surfaces.get(module_id)
    }

    /// Drop one module's surface (its instance was bypassed or removed — the band falls back
    /// to at rest, honestly).
    pub fn remove(&mut self, module_id: &str) {
        self.surfaces.remove(module_id);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// A scripted instance: `script` is popped per draw — `Ok(())` yields an empty (but LIVE)
    /// surface, `Err(())` is an overrun with §3-class words. The pacer's contract is the
    /// outcome and the frame, not the surface's contents.
    struct Mock {
        script: Vec<Result<(), ()>>,
        drawn: Vec<DisplayFrame>,
    }
    impl Mock {
        fn new(script: Vec<Result<(), ()>>) -> Self {
            Self { script, drawn: Vec::new() }
        }
    }
    impl LiveDisplay for Mock {
        fn draw(&mut self, frame: &DisplayFrame) -> Result<Vec<Item>, String> {
            self.drawn.push(frame.clone());
            match self.script.pop() {
                Some(Ok(())) => Ok(Vec::new()),
                Some(Err(())) => {
                    Err("fuel budget exhausted (99 of 50 burned) — the guest overran its budget"
                        .to_string())
                },
                None => Ok(Vec::new()),
            }
        }
        fn display_id(&self) -> &str {
            "wall"
        }
    }

    fn frame(w: u32, h: u32, t: f64) -> DisplayFrame {
        DisplayFrame {
            display_id: "wall".to_string(),
            width_px: w,
            height_px: h,
            lod: DisplayLod::Full,
            time_sec: t,
        }
    }

    #[test]
    fn the_pacer_paces_at_the_d19_token() {
        let p = DisplayPacer::new();
        assert_eq!(p.period_ms(), 66, "1000/15 = 66 ms — the token's arithmetic");
        // A never-seen node draws on the first frame.
        assert!(p.due(1, 0));
        let mut p = p;
        p.record(1, true, 0);
        assert!(!p.due(1, 65), "65 ms in: not yet");
        assert!(p.due(1, 66), "66 ms: due — 15 Hz");
        // ~4 frames of a 60 Hz shell per draw: the pace, not the frame rate, governs.
        p.record(1, true, 66);
        assert!(!p.due(1, 100) && p.due(1, 132));
    }

    #[test]
    fn an_edit_forces_the_next_frame_regardless_of_the_pace() {
        let mut p = DisplayPacer::new();
        p.record(1, true, 0);
        assert!(!p.due(1, 16), "16 ms in: paced");
        p.note_edit(1);
        assert!(p.due(1, 16), "the edit's effect is immediate (D19)");
        p.record(1, true, 16);
        assert!(!p.due(1, 32), "…and the pace resumes after the forced draw");
        // An LOD change forces every node.
        p.record(2, true, 0);
        p.note_lod_change();
        assert!(p.due(1, 20) && p.due(2, 20));
    }

    #[test]
    fn five_consecutive_overruns_bypass_and_an_edit_rearms() {
        let mut p = DisplayPacer::new();
        let mut inst = Mock::new(vec![Err(()); 10]);
        // Five failures in a row: each SKIPS the frame (the surface keeps the last good), the
        // fifth bypasses — and a bypassed instance is never due again.
        for i in 0..BYPASS_AFTER {
            let now = u64::from(i) * 100;
            assert!(p.due(1, now), "attempt {} is drawn", i + 1);
            let r = inst.draw(&frame(100, 100, 0.0));
            p.record(1, r.is_ok(), now);
        }
        assert!(p.bypassed(1), "five consecutive overruns bypass the DISPLAY instance (D17c)");
        assert!(!p.due(1, 10_000), "a bypassed instance draws no more");
        // A node that left the canvas is forgotten — the graph is the truth, and a re-spawn
        // starts fresh (the tick's cleanup calls this).
        p.forget(1);
        assert!(p.due(1, 10_000), "a forgotten node is new again");
        // The audio side is untouched by construction: the pacer knows nothing of it (decision
        // D's isolation — two watchdogs, never one mechanism).
        // An edit re-arms (the retry door).
        p.note_edit(1);
        assert!(!p.bypassed(1) && p.due(1, 10_001));
        // A success resets the count: four failures then a success then four more = no bypass.
        let mut p2 = DisplayPacer::new();
        for i in 0..4u64 {
            p2.record(2, false, i);
        }
        p2.record(2, true, 10);
        for i in 0..4u64 {
            p2.record(2, false, 20 + i);
        }
        assert!(!p2.bypassed(2), "CONSECUTIVE — a success in between resets the count");
    }

    #[test]
    fn the_live_store_switches_the_band_shelf() {
        let mut live = LiveSurfaces::new();
        assert!(live.get("dat/observatory").is_none(), "an empty store is all fallback");
        live.insert(
            "dat/observatory",
            LiveSurface { items: Vec::new(), w: 1088.0, h: 560.0, drawn_ms: 42 },
        );
        let s = live.get("dat/observatory").unwrap();
        assert_eq!((s.w, s.h, s.drawn_ms), (1088.0, 560.0, 42), "the band's actual px (O-1)");
        live.remove("dat/observatory");
        assert!(live.get("dat/observatory").is_none(), "removed: the band falls back to at rest");
    }

    #[test]
    fn the_mock_draws_what_the_frame_says() {
        // The frame-context contract: the band's actual px and the animation clock reach the
        // instance verbatim (the device adapter fills the WIT struct from exactly these).
        let mut inst = Mock::new(vec![Ok(())]);
        let f = frame(1088, 560, 12.5);
        let items = inst.draw(&f).unwrap();
        assert!(items.is_empty(), "a live-but-empty surface is still LIVE (the shelf switched)");
        assert_eq!(inst.drawn[0], f);
        assert_eq!(inst.display_id(), "wall");
        // The LOD's three rungs all cross the seam (the guest's §5.7 ladder reads them).
        for lod in [DisplayLod::Full, DisplayLod::Simplified, DisplayLod::Dot] {
            let mut f2 = frame(100, 100, 0.0);
            f2.lod = lod;
            assert!(inst.draw(&f2).is_ok());
            assert_eq!(inst.drawn.last().unwrap().lod, lod);
        }
    }

    #[test]
    fn the_bypass_words_are_the_section_3_sentence() {
        // The words are a recorded promise (§3's mapping row: frames dropped and counted, N
        // consecutive → the DISPLAY instance bypassed, the panel shows words, the audio
        // instance untouched) — pinned so a rewrite cannot quietly soften them.
        assert!(BYPASS_WORDS.contains("DISPLAY BYPASSED"));
        assert!(BYPASS_WORDS.contains("five consecutive"));
        assert!(BYPASS_WORDS.contains("audio instance is untouched"), "{BYPASS_WORDS}");
        assert_eq!(BYPASS_AFTER, 5, "the plan's own number");
    }
}
