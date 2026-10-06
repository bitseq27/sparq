//! The module browser: fuzzy search over a catalogue, ranked rows, sheet geometry (WO-013
//! increment 3).
//!
//! Command-surface v0 from the work order: the user long-presses the empty canvas, chooses
//! **ADD MODULE**, types (or scrolls) and taps a row — the node appears where they pressed. Like
//! everything in `sparq-ui`, this file is **computed, not drawn**: it owns the ranking, the
//! selection and the row geometry; the painter in `sparq-app` renders exactly these rects, and
//! the headless audit hit-tests exactly these rects, so the row you see is the row you touch.
//!
//! The catalogue is *input*, never a lookup: the shell hands the browser the [`BrowserItem`]s it
//! built from the registry, so the browser cannot offer a module that is not installed (defect
//! #58's rule, structurally). Text entry is the toolkit's job (an egui `TextEdit` in the window
//! shell); the core only consumes the query string, which keeps it testable headless and free of
//! keyboard/focus policy.

use crate::canvas::model::NodeSpec;
use crate::geom::{Rect, Vec2};
use crate::tokens::{LAYOUT_SPACE_2, LAYOUT_SPACE_3, LAYOUT_TOUCH_ROW_HEIGHT_BROWSER};
use sparq_module_api::manifest::Layer;

/// One catalogue entry: everything needed to spawn the module and to show it in a row.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserItem {
    /// The node spec a spawn inserts — ports AND params, the registry's validated views.
    pub spec: NodeSpec,
    /// The manifest's one-line summary (may be empty; the row degrades gracefully).
    pub summary: String,
    /// The manifest's category, e.g. `synth/oscillator/sine` — searchable, shown dim.
    pub category: String,
    /// The library layer this module plays in (ADR-010, contract v1.1): the data the browser
    /// groups by. Carried as DATA only — ranking and row geometry do not read it, so the rows
    /// stay exactly as the audit pinned them until the instrument chrome (visual grouping,
    /// badging) lands with the WO-018 host.
    pub layer: Layer,
}

impl BrowserItem {
    /// An item from a spec alone (tests, and catalogues without manifest prose). The layer rides
    /// the SPEC (WO-020 INC4: the browser groups by it) — an item that re-defaulted it would file
    /// an instrument under the backbone.
    #[must_use]
    pub fn new(spec: NodeSpec) -> Self {
        Self {
            spec: spec.clone(),
            summary: String::new(),
            category: String::new(),
            layer: spec.layer,
        }
    }

    /// The row's primary text: the display name and the module id.
    #[must_use]
    pub fn row_text(&self) -> String {
        format!("{}  {}", self.spec.display_name, self.spec.module_id)
    }

    /// The text the fuzzy matcher searches, best field first: display name, module id, category.
    /// (The summary is deliberately NOT searched — prose matches drown the id matches that mean
    /// something; the row still *shows* the summary.)
    fn haystacks(&self) -> [&str; 3] {
        [&self.spec.display_name, &self.spec.module_id, &self.category]
    }
}

/// The browser's group sections (WO-020 INC4 §8.2, the WO-018-note "browser visual grouping"):
/// instruments group under their own header, the backbone under its own — rank order preserved
/// WITHIN each section, and the section order is instruments-first (the layer a visitor came to
/// see). The headers are rows in the sheet (touch floor honoured by the drawer); an empty section
/// draws no header.
#[must_use]
pub fn sections(items: Vec<BrowserItem>) -> [(&'static str, Vec<BrowserItem>); 2] {
    let (inst, back): (Vec<BrowserItem>, Vec<BrowserItem>) =
        items.into_iter().partition(|i| i.layer == sparq_module_api::manifest::Layer::Instrument);
    [("INSTRUMENTS", inst), ("MODULES", back)]
}

/// A group-header sentinel row (WO-020 INC4 §8.2): the empty module id marks it — the drawer
/// paints it as a dim uppercase title, and a tap on it refuses in words (a header is not a
/// module). Headers ride the ranked list so the sheet's paging, scroll and touch geometry stay
/// exactly as the audit pinned them.
#[must_use]
pub fn header_item(title: &'static str) -> BrowserItem {
    BrowserItem {
        spec: NodeSpec::new("", title, vec![]),
        summary: String::new(),
        category: String::new(),
        layer: Layer::Backbone,
    }
}

/// Whether a catalogue row is a group header sentinel.
#[must_use]
pub fn is_header(item: &BrowserItem) -> bool {
    item.spec.module_id.is_empty()
}

/// Groups a catalogue: instruments under their own header FIRST (the layer a visitor came to
/// see), the backbone under its own — each in its given (rank) order. With a query the ranker
/// drops headers (an empty haystack matches nothing), which is right: a search is a flat answer.
#[must_use]
pub fn with_sections(items: Vec<BrowserItem>) -> Vec<BrowserItem> {
    let (inst, back): (Vec<BrowserItem>, Vec<BrowserItem>) =
        items.into_iter().partition(|i| i.layer == Layer::Instrument);
    let mut out = Vec::new();
    if !inst.is_empty() {
        out.push(header_item("INSTRUMENTS"));
        out.extend(inst);
    }
    if !back.is_empty() {
        out.push(header_item("MODULES"));
        out.extend(back);
    }
    out
}

/// Score a `query` against one `text`: `Some(score)` when every query character appears in the
/// text **in order** (case-insensitive subsequence), else `None`. Higher is better; the scale is
/// internal — only the ordering is a contract.
///
/// The ranking rules, in the order they bite (all deterministic, no allocation, no hashing):
///
/// 1. **Contiguity** — a matched run of consecutive characters scores per-char plus a run bonus,
///    so `sin` finds **Sin**e before a **s**pread-**i**nto-**n**ine coincidence.
/// 2. **Word starts** — a match at the start of the text or right after a separator
///    (`/ - _ space .`) scores a boundary bonus, so `g` finds util/**g**ain before bi**g**.
/// 3. **Earliness** — the position of the first match is subtracted, so a prefix beats a buried
///    subsequence of the same shape.
///
/// Field order breaks ties before the catalogue order does (see [`rank`]): the display name
/// outranks the id, the id outranks the category.
#[must_use]
pub fn fuzzy_score(query: &str, text: &str) -> Option<i64> {
    if query.is_empty() {
        // An empty query matches everything; the rank order then falls back to the catalogue.
        return Some(0);
    }
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let mut best: Option<i64> = None;
    // Scan every start position; a greedy match from each gives the highest-scoring alignment
    // for that start under rules 1–3 (greedy takes the earliest possible continuation, which
    // maximises contiguity and earliness together). Catalogues are tens of rows; texts are short.
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    for start in 0..t.len() {
        let Some(score) = score_from(&q, &t, start) else { continue };
        best = Some(best.map_or(score, |b| b.max(score)));
    }
    best
}

/// Greedy subsequence match beginning exactly at `t[start]`; `None` if `t[start]` is not `q[0]`
/// or the rest does not fit in order.
fn score_from(q: &[char], t: &[char], start: usize) -> Option<i64> {
    if t[start] != q[0] {
        return None;
    }
    let mut score: i64 = 0;
    let mut qi = 0usize;
    let mut ti = start;
    let mut run = 0i64;
    while qi < q.len() && ti < t.len() {
        if t[ti] == q[qi] {
            // per-char point
            score += 1;
            // contiguity: this match directly follows the previous one in BOTH strings. The bonus
            // outweighs a word-start (below), so a run in one word beats the same letters spread
            // over several word starts — "sin" finds **Sin**e, not "**S**et **In** Nine".
            if qi > 0 && run > 0 {
                score += 6;
            }
            run = if qi > 0 { run + 1 } else { 1 };
            // word start: index 0 or right after a separator
            if ti == 0 || is_sep(t[ti - 1]) {
                score += 4;
            }
            qi += 1;
            ti += 1;
        } else {
            run = 0;
            ti += 1;
        }
    }
    if qi < q.len() {
        return None; // query did not fit
    }
    // earliness: subtract the first match position (bounded below so deep matches stay negative
    // but comparable)
    Some(score - start as i64)
}

fn is_sep(c: char) -> bool {
    matches!(c, '/' | '-' | '_' | ' ' | '.' | ':')
}

/// Rank catalogue indices against `query`: matched items first, best score first, ties broken by
/// catalogue position (stable, deterministic — the same catalogue and query always give the same
/// list, which the audit and the goldens rely on). Unmatched items are **excluded**; an empty
/// query returns the whole catalogue in order.
#[must_use]
pub fn rank(items: &[BrowserItem], query: &str) -> Vec<usize> {
    let mut scored: Vec<(i64, usize)> = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        let best = if query.is_empty() {
            Some(0)
        } else {
            it.haystacks()
                .iter()
                // field order bonus: name (0) > id (1) > category (2), worth more than any
                // single positional point but never more than the contiguity/boundary structure
                .enumerate()
                .filter_map(|(f, h)| fuzzy_score(query, h).map(|s| s - i64::from(f as u8)))
                .max()
        };
        if let Some(s) = best {
            scored.push((s, i));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// The open browser: query, ranking, selection, scroll, and where the sheet sits.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserState {
    /// The catalogue (owned; the shell rebuilds it when the registry changes, which in Phase 0
    /// is only at startup).
    items: Vec<BrowserItem>,
    query: String,
    ranked: Vec<usize>,
    /// Selected position **within the ranked list** (not the page).
    selected: usize,
    /// First ranked position shown on the page (scroll), kept so `selected` stays visible.
    scroll: usize,
    /// Where the long-press landed, screen px — the sheet anchors here, clamped into the view.
    pub anchor_screen: Vec2,
    /// Where the chosen module will spawn, **world** px (the anchor, un-projected, by the
    /// opener). Snapping and cascade-on-overlap happen at spawn time, not here.
    pub spawn_world: Vec2,
}

impl BrowserState {
    /// Open on a catalogue with an empty query (everything listed, catalogue order).
    #[must_use]
    pub fn open(items: Vec<BrowserItem>, anchor_screen: Vec2, spawn_world: Vec2) -> Self {
        let ranked = rank(&items, "");
        Self {
            items,
            query: String::new(),
            ranked,
            selected: 0,
            scroll: 0,
            anchor_screen,
            spawn_world,
        }
    }

    /// The catalogue, in ranked order.
    pub fn visible(&self) -> impl Iterator<Item = &BrowserItem> {
        self.ranked.iter().map(|&i| &self.items[i])
    }

    /// How many rows match.
    #[must_use]
    pub fn visible_len(&self) -> usize {
        self.ranked.len()
    }

    /// The current query.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Replace the query and re-rank. Selection resets to the best match; the page scrolls back
    /// to the top. (Typing always shows you the new best first — the command-surface behaviour.)
    pub fn set_query(&mut self, q: &str) {
        self.query = q.to_string();
        self.ranked = rank(&self.items, &self.query);
        self.selected = 0;
        self.scroll = 0;
    }

    /// The selected item, if the ranking is non-empty.
    #[must_use]
    pub fn selected_item(&self) -> Option<&BrowserItem> {
        self.ranked.get(self.selected).map(|&i| &self.items[i])
    }

    /// The selected position within the ranked list.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The first ranked position shown on the page.
    #[must_use]
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Move the selection by `delta` (negative = up), clamped to the ranked range, keeping the
    /// selection inside the `page_rows`-tall window by scrolling the minimum amount.
    pub fn move_selection(&mut self, delta: i64, page_rows: usize) {
        if self.ranked.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        let n = self.ranked.len() as i64;
        self.selected = (self.selected as i64 + delta).clamp(0, n - 1) as usize;
        let page = page_rows.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + page {
            self.scroll = self.selected - page + 1;
        }
    }

    /// Select rank position `i` directly (a row tap), clamped into range. Returns `false` if the
    /// list is empty.
    pub fn select(&mut self, i: usize) -> bool {
        if i < self.ranked.len() {
            self.selected = i;
            true
        } else {
            false
        }
    }

    // --------------------------------------------------------------- geometry (screen px)

    /// The sheet width: the widest row (header included), on the same monospace-advance budget
    /// the context menu uses — clamped to `max_w` so a long summary cannot overrun the view.
    #[must_use]
    pub fn width(&self, max_w: f32) -> f32 {
        let char_w = LAYOUT_SPACE_2 as f32;
        let pad = LAYOUT_SPACE_3 as f32 * 2.0;
        let header = self.query.len().max(7) + 2; // "SEARCH " prefix
        let longest = self
            .visible()
            .take(12) // measure a page, not a 200-module catalogue
            .map(|it| it.row_text().chars().count().max(it.summary.chars().count()))
            .max()
            .unwrap_or(0)
            .max(header);
        (pad + longest as f32 * char_w).min(max_w)
    }

    /// The sheet height for `page_rows` rows: header row + the rows actually shown.
    #[must_use]
    pub fn height(&self, page_rows: usize) -> f32 {
        let rows = page_rows.min(self.ranked.len()).max(1); // ≥1: an empty ranking still shows
                                                            // the header and a NO MATCH row
        (rows as f32 + 1.0) * LAYOUT_TOUCH_ROW_HEIGHT_BROWSER as f32
    }

    /// The clamped top-left of the sheet inside `view`, at most `max_w` wide and `max_h` tall.
    #[must_use]
    pub fn sheet_origin(&self, view: Rect, max_w: f32, max_h: f32) -> Vec2 {
        let page = Self::page_rows(max_h);
        let size = Vec2::new(self.width(max_w), self.height(page).min(max_h));
        crate::canvas::interact::clamp_origin(self.anchor_screen, size, view)
    }

    /// How many rows fit in `max_h` (always ≥ 1).
    #[must_use]
    pub fn page_rows(max_h: f32) -> usize {
        let row = LAYOUT_TOUCH_ROW_HEIGHT_BROWSER as f32;
        ((max_h - row) / row).floor().max(1.0) as usize // minus the header row
    }

    /// The header (query display / text-entry) rect, given the sheet `origin`.
    #[must_use]
    pub fn header_rect(&self, origin: Vec2, view: Rect, max_w: f32) -> Rect {
        let row = LAYOUT_TOUCH_ROW_HEIGHT_BROWSER as f32;
        Rect::from_min_size(origin, Vec2::new(self.width(max_w).min(view.width()), row))
    }

    /// The rect of page row `i` (0 = the first VISIBLE row, i.e. rank `scroll + i`), given the
    /// sheet `origin`. The painter and the hit-test both call this.
    #[must_use]
    pub fn row_rect(&self, origin: Vec2, i: usize, view: Rect, max_w: f32) -> Rect {
        let row = LAYOUT_TOUCH_ROW_HEIGHT_BROWSER as f32;
        Rect::from_min_size(
            Vec2::new(origin.x, origin.y + row * (i as f32 + 1.0)),
            Vec2::new(self.width(max_w).min(view.width()), row),
        )
    }

    /// What a screen point hits inside the sheet: the header, a page row (returning the rank
    /// position), or nothing. With an empty ranking the single NO-MATCH row slot is still
    /// hittable — it maps to rank 0, which [`BrowserState::select`] refuses, so the canvas can
    /// answer the tap with the "nothing matches" refusal instead of a mysterious no-op (the row
    /// [`Self::height`] reserves and the row [`Self::hit`] accepts stay the same row).
    #[must_use]
    pub fn hit(&self, pos: Vec2, view: Rect, max_w: f32, max_h: f32) -> BrowserHit {
        let origin = self.sheet_origin(view, max_w, max_h);
        let page = Self::page_rows(max_h);
        if self.header_rect(origin, view, max_w).contains(pos) {
            return BrowserHit::Query;
        }
        for i in 0..page.min(self.ranked.len().max(1)) {
            if self.row_rect(origin, i, view, max_w).contains(pos) {
                return BrowserHit::Row(self.scroll + i);
            }
        }
        BrowserHit::Outside
    }
}

/// What a tap inside the browser sheet landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserHit {
    /// The query header (the shell focuses its text entry there).
    Query,
    /// A row, by position **in the ranked list**.
    Row(usize),
    /// Outside the sheet (the canvas closes the browser, like the context menu).
    Outside,
}

/// The sheet's size caps as fractions of the canvas view — ONE contract shared by the hit-test
/// (`interact`), the tests and the painter (`sparq-app`), so what is touchable and what is drawn
/// agree by construction.
#[must_use]
pub fn caps(view: Rect) -> (f32, f32) {
    (view.width() * 0.5, view.height() * 0.5)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::canvas::model::NodeSpec;

    fn item(id: &str, name: &str, category: &str) -> BrowserItem {
        BrowserItem {
            spec: NodeSpec::new(id, name, vec![]),
            summary: String::new(),
            category: category.into(),
            layer: Layer::default(),
        }
    }

    fn catalogue() -> Vec<BrowserItem> {
        vec![
            item("sparq/syn/sine", "Sine", "synth/oscillator/sine"),
            item("sparq/util/gain", "Gain", "utility/gain"),
            item("sparq/ana/rms", "RMS", "analysis/rms"),
        ]
    }

    #[test]
    fn the_layer_is_data_only_ranking_does_not_read_it() {
        // The audit-neutrality promise of the WO-017 close: visual grouping is WO-018's chrome,
        // so until it lands, two catalogues that differ ONLY in `layer` must rank identically —
        // every geometry the headless audit pinned stays pinned.
        let a = catalogue();
        let mut b = catalogue();
        b[0].layer = Layer::Instrument;
        for q in ["", "sin", "g", "rms", "s"] {
            assert_eq!(rank(&a, q), rank(&b, q), "layer moved the ranking for {q:?}");
        }
        assert_eq!(a[0].row_text(), b[0].row_text(), "layer moved the row text");
    }

    // ---------------------------------------------------------------- fuzzy

    #[test]
    fn fuzzy_matches_subsequences_in_order_and_refuses_the_rest() {
        assert!(fuzzy_score("si", "Sine").is_some());
        assert!(fuzzy_score("sn", "Sine").is_some(), "s…n is in order");
        assert!(fuzzy_score("ns", "Sine").is_none(), "n before s is not a subsequence");
        assert!(fuzzy_score("", "Sine").is_some(), "empty query matches everything");
        assert!(fuzzy_score("sin", "").is_none());
    }

    #[test]
    fn fuzzy_is_case_insensitive() {
        assert_eq!(fuzzy_score("SIN", "sine"), fuzzy_score("sin", "SINE"));
    }

    #[test]
    fn contiguity_outranks_a_spread_subsequence() {
        let together = fuzzy_score("sin", "Sine").expect("matches");
        let spread = fuzzy_score("sin", "Set In Nine").expect("matches");
        assert!(together > spread, "run {together} vs spread {spread}");
    }

    #[test]
    fn word_starts_outrank_mid_word_matches() {
        let start = fuzzy_score("g", "gain").expect("matches");
        let buried = fuzzy_score("g", "bigger").expect("matches");
        assert!(start > buried);
    }

    #[test]
    fn separators_count_as_word_starts() {
        let after_sep = fuzzy_score("g", "util/gain").expect("matches");
        let buried = fuzzy_score("g", "bigger").expect("matches");
        assert!(after_sep > buried, "/g beats a buried g");
    }

    // ---------------------------------------------------------------- ranking

    #[test]
    fn an_empty_query_lists_the_whole_catalogue_in_order() {
        let c = catalogue();
        assert_eq!(rank(&c, ""), vec![0, 1, 2]);
    }

    #[test]
    fn the_best_match_comes_first() {
        let c = catalogue();
        let r = rank(&c, "si");
        assert_eq!(r[0], 0, "Sine wins for 'si'");
        let r = rank(&c, "ga");
        assert_eq!(r[0], 1, "Gain wins for 'ga'");
    }

    #[test]
    fn the_display_name_outranks_the_id_and_the_id_outranks_the_category() {
        let c = catalogue();
        // 'rms' matches Sine's nothing, Gain's nothing, and RMS's name+id.
        let r = rank(&c, "rms");
        assert_eq!(r, vec![2]);
        // 'util' only appears in the gain item's id.
        let r = rank(&c, "util");
        assert_eq!(r, vec![1]);
        // 'oscillator' only appears in a category.
        let r = rank(&c, "osc");
        assert_eq!(r, vec![0]);
    }

    #[test]
    fn unmatched_items_are_excluded() {
        let c = catalogue();
        assert!(rank(&c, "zzz").is_empty());
    }

    #[test]
    fn ranking_is_deterministic_for_ties() {
        let c = catalogue();
        // 'a' appears in all three (cAtegories too); two runs must agree exactly.
        assert_eq!(rank(&c, "a"), rank(&c, "a"));
    }

    // ---------------------------------------------------------------- state

    #[test]
    fn set_query_reranks_and_resets_the_selection() {
        let mut b = BrowserState::open(catalogue(), Vec2::new(100.0, 100.0), Vec2::ZERO);
        b.move_selection(2, 10);
        assert_eq!(b.selected(), 2);
        b.set_query("ga");
        assert_eq!(b.selected(), 0, "typing shows the best match first");
        assert_eq!(b.selected_item().unwrap().spec.module_id, "sparq/util/gain");
        assert_eq!(b.visible_len(), 1);
    }

    #[test]
    fn selection_moves_stay_in_range_and_scroll_keeps_them_visible() {
        let mut b = BrowserState::open(catalogue(), Vec2::ZERO, Vec2::ZERO);
        b.move_selection(-5, 2); // clamps at 0
        assert_eq!(b.selected(), 0);
        b.move_selection(2, 2); // clamps at len-1 = 2
        assert_eq!(b.selected(), 2);
        assert_eq!(b.scroll(), 1, "page of 2 scrolled so rank 2 is visible");
        b.move_selection(-2, 2);
        assert_eq!(b.selected(), 0);
        assert_eq!(b.scroll(), 0, "scrolled back up");
    }

    #[test]
    fn a_query_with_no_matches_selects_nothing_and_says_so_by_length() {
        let mut b = BrowserState::open(catalogue(), Vec2::ZERO, Vec2::ZERO);
        b.set_query("zzz");
        assert_eq!(b.visible_len(), 0);
        assert!(b.selected_item().is_none());
    }

    // ---------------------------------------------------------------- geometry

    fn view() -> Rect {
        Rect::from_min_size(Vec2::ZERO, Vec2::new(1200.0, 800.0))
    }

    #[test]
    fn rows_clear_the_touch_minimum_and_the_page_row_count_is_honest() {
        assert!(
            LAYOUT_TOUCH_ROW_HEIGHT_BROWSER as f32 >= 44.0,
            "browser rows must clear the class-S touch minimum"
        );
        // 800 px view → header + floor((800-56)/56) rows
        assert_eq!(BrowserState::page_rows(800.0), 13);
        assert_eq!(BrowserState::page_rows(56.0), 1, "never fewer than one row");
    }

    #[test]
    fn the_sheet_stays_inside_the_view() {
        let b = BrowserState::open(catalogue(), Vec2::new(1190.0, 790.0), Vec2::ZERO);
        let o = b.sheet_origin(view(), 480.0, 600.0);
        let w = b.width(480.0);
        assert!(o.x + w <= 1200.0 + f32::EPSILON, "clamped horizontally");
        assert!(o.y >= 0.0 && o.y < 800.0);
    }

    #[test]
    fn row_hit_testing_round_trips_the_rank_position() {
        let mut b = BrowserState::open(catalogue(), Vec2::new(200.0, 200.0), Vec2::ZERO);
        let origin = b.sheet_origin(view(), 480.0, 600.0);
        // tap the centre of page row 1 → rank 1 (scroll is 0)
        let r = b.row_rect(origin, 1, view(), 480.0);
        assert_eq!(b.hit(r.center(), view(), 480.0, 600.0), BrowserHit::Row(1));
        // header
        let h = b.header_rect(origin, view(), 480.0);
        assert_eq!(b.hit(h.center(), view(), 480.0, 600.0), BrowserHit::Query);
        // outside
        assert_eq!(b.hit(Vec2::new(5.0, 795.0), view(), 480.0, 600.0), BrowserHit::Outside);
        // scrolled: move the selection past a 2-row page, then the first page row is rank 1
        b.move_selection(2, 2);
        assert_eq!(b.scroll(), 1);
        let origin = b.sheet_origin(view(), 480.0, 600.0);
        let r0 = b.row_rect(origin, 0, view(), 480.0);
        assert_eq!(b.hit(r0.center(), view(), 480.0, 600.0), BrowserHit::Row(1));
    }
}
