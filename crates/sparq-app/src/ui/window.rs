//! The window host (WO-012 task 1): winit event loop → normalised pointers → the same
//! [`ShellUi::frame`] the headless driver runs → egui-wgpu paint.
//!
//! DPI: winit sets Per-Monitor-V2 DPI awareness on Windows itself and delivers every size in
//! physical pixels plus a `scale_factor`. This module divides by the scale factor **once**, at
//! the boundary, and everything downstream (gestures, layout, audit) lives in logical px — that
//! single transform is the whole per-monitor-DPI story, and `docs/ui/windows-dpi-notes.md`
//! records what SATURN measures of it.
//!
//! Known limitation, stated honestly: winit's `Touch` event carries no contact area, so the palm
//! threshold never triggers from real touches yet — palm rejection is proven with synthetic
//! areas (headless suite) and needs `GetPointerFrameInfo`-level detail (WM_POINTER) for the real
//! digitiser. That is WO-012 increment 2, recorded in the build log, not silently skipped.

use std::sync::Arc;
use std::time::Instant;

use egui::ViewportId;
use sparq_ui::geom::Vec2 as SpVec2;
use sparq_ui::gesture::GestureIntent;
use sparq_ui::pointer::{PointerEvent, PointerKind, PointerPhase};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{
    ElementState, Force, MouseButton, MouseScrollDelta, TouchPhase as WTouchPhase, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::ui::adapter::{self, Palette, ThemeChoice};
use crate::ui::shell_ui::{FrameInput, ShellUi};
use crate::ui::UiOptions;

/// The winit application: one window, one egui context, one shell.
struct ShellApp {
    ctx: egui::Context,
    egui_winit: Option<egui_winit::State>,
    painter: Option<egui_wgpu::winit::Painter>,
    window: Option<Arc<Window>>,
    shell: ShellUi,
    /// Normalised pointer events accumulated since the last frame.
    pointers: Vec<PointerEvent>,
    /// Last known cursor position (logical px), for synthesising mouse down/up events.
    cursor_logical: Option<SpVec2>,
    mouse_down: bool,
    /// The right button's drag state (operator ruling 2026-10-01: right-drag MOVES THE
    /// CANVAS; a right press that never moves stays the context menu — the desktop hand's
    /// long-press, unchanged).
    right_down: bool,
    right_start: Option<SpVec2>,
    right_last: Option<SpVec2>,
    right_moved: bool,
    /// Intents synthesised outside the recogniser (right-click, wheel) — drained per frame.
    extras: Vec<GestureIntent>,
    start: Instant,
    theme: ThemeChoice,
    /// Set when wgpu failed to initialise; the loop then exits with an explanation instead of
    /// silently drawing nothing (an empty window on stage is a failure that looks like a hang).
    fatal: Option<String>,
}

/// One wheel notch's zoom factor (operator ruling 2026-10-01: the mouse scrollwheel ZOOMS the
/// canvas about the cursor — the pinch's mouse hand, riding the recogniser's own `Zoom`
/// vocabulary so the camera's clamp and anchor are the one implementation).
const WHEEL_ZOOM_STEP: f32 = 1.15;
/// Per-pixel wheel zoom for high-resolution (touchpad) deltas, clamped so one coalesced event
/// can never jump the camera past a sane step.
const WHEEL_ZOOM_PER_PX: f32 = 0.005;
/// How far a right press must travel before it is a canvas drag and not a context menu — the
/// recogniser's own drag threshold (8 px), so the mouse and the finger agree on the word "drag".
const RIGHT_DRAG_MIN_PX: f32 = 8.0;

impl ShellApp {
    fn new(opts: &UiOptions) -> Self {
        let ctx = egui::Context::default();
        let theme =
            if opts.contrast { ThemeChoice::ContrastHigh } else { ThemeChoice::PhosphorDark };
        adapter::apply_style(&ctx, theme);
        let mut shell = ShellUi::new();
        shell.theme = theme;
        Self {
            ctx,
            egui_winit: None,
            painter: None,
            window: None,
            shell,
            pointers: Vec::new(),
            cursor_logical: None,
            mouse_down: false,
            right_down: false,
            right_start: None,
            right_last: None,
            right_moved: false,
            extras: Vec::new(),
            start: Instant::now(),
            theme,
            fatal: None,
        }
    }

    fn scale(&self) -> f64 {
        self.window.as_ref().map_or(1.0, |w| w.scale_factor())
    }

    fn to_logical(&self, x: f64, y: f64) -> SpVec2 {
        let s = self.scale();
        SpVec2::new((x / s) as f32, (y / s) as f32)
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn push_pointer(
        &mut self,
        id: u64,
        pos: SpVec2,
        pressure: f32,
        kind: PointerKind,
        phase: PointerPhase,
    ) {
        self.pointers.push(PointerEvent {
            id,
            pos,
            pressure,
            // winit reports no contact area; see the module header (increment 2: WM_POINTER).
            contact_area_mm2: 0.0,
            kind,
            phase,
            t_ms: self.now_ms(),
        });
    }
}

impl ApplicationHandler for ShellApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("sparq - WO-012 shell prototype")
            .with_inner_size(PhysicalSize::new(1600.0, 900.0))
            .with_min_inner_size(PhysicalSize::new(400.0, 300.0));
        let Ok(window) = event_loop.create_window(attrs) else {
            self.fatal = Some("could not create a window (no display?)".to_string());
            event_loop.exit();
            return;
        };
        let window = Arc::new(window);

        // egui-wgpu's setup is async (wgpu instance + adapter + device); pollster's block_on is
        // the whole executor story for a UI process that has exactly one thing to wait for.
        let mut painter = pollster::block_on(egui_wgpu::winit::Painter::new(
            self.ctx.clone(),
            egui_wgpu::WgpuConfiguration::default(),
            false,
            egui_wgpu::RendererOptions::default(),
        ));
        if let Err(e) =
            pollster::block_on(painter.set_window(ViewportId::ROOT, Some(window.clone())))
        {
            self.fatal = Some(format!(
                "wgpu could not initialise a surface: {e:?} — on an RDP session this usually means no usable GPU/WARP adapter"
            ));
            event_loop.exit();
            return;
        }

        let egui_winit = egui_winit::State::new(
            self.ctx.clone(),
            ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            painter.max_texture_side(),
        );

        self.window = Some(window.clone());
        self.painter = Some(painter);
        self.egui_winit = Some(egui_winit);
        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if let Some(ew) = self.egui_winit.as_mut() {
            // egui-winit tracks DPI/size/IME state from every window event; its EventResponse
            // (consumed/repaint-requested) is advisory — this loop repaints continuously.
            let _ = ew.on_window_event(&window, &event);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput { event: ke, .. } => {
                if ke.state == ElementState::Pressed {
                    match ke.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                        // DELETE removes the selection (operator ruling 2026-10-01): the
                        // canvas owns the answer, including its protections (locked nodes and
                        // the permanent Main Out refuse in words) — the key only carries the
                        // intent, the recogniser's own vocabulary. While the library's search
                        // field has the keyboard, the key belongs to the text, not the canvas.
                        PhysicalKey::Code(KeyCode::Delete) if !self.shell.library_focus => {
                            self.extras.push(GestureIntent::Delete);
                        },
                        _ => {},
                    }
                }
            },

            // ---- touch: the primary input class. Physical → logical happens HERE and only here.
            WindowEvent::Touch(t) => {
                let pos = self.to_logical(t.location.x, t.location.y);
                let phase = match t.phase {
                    WTouchPhase::Started => PointerPhase::Down,
                    WTouchPhase::Moved => PointerPhase::Moved,
                    WTouchPhase::Ended => PointerPhase::Up,
                    WTouchPhase::Cancelled => PointerPhase::Cancel,
                };
                // A liftoff carries no meaningful pressure, and reporting 1.0 there would make
                // `is_contact()` call the release a contact — which would keep recognisers in a
                // phase they should have left. Same reason mouse-up reports 0.0 below.
                let pressure = if phase == PointerPhase::Up {
                    0.0
                } else {
                    match t.force {
                        Some(Force::Normalized(v)) => v as f32,
                        Some(Force::Calibrated { force, max_possible_force, .. }) => {
                            (force / max_possible_force.max(1e-9)) as f32
                        },
                        // Most Windows digitisers report no force through winit. Absence of a
                        // reading is not absence of a finger: a contact that reported 0.0 would
                        // vanish from the recogniser mid-gesture (see pointer.rs `is_contact`).
                        None => 1.0,
                    }
                };
                self.push_pointer(t.id, pos, pressure, PointerKind::Finger, phase);
            },

            // ---- mouse: hover is a zero-pressure pointer that never enters Down (input-model §2)
            WindowEvent::CursorMoved { position, .. } => {
                let pos = self.to_logical(position.x, position.y);
                self.cursor_logical = Some(pos);
                // Right-drag MOVES THE CANVAS (operator ruling 2026-10-01): while the right
                // button is down, motion past the drag threshold becomes the recogniser's own
                // `Pan` — over the canvas the camera follows the hand, over a scrolling panel
                // the panel scrolls (the Pan arm's existing routing; no parallel semantics).
                if self.right_down {
                    let start = self.right_start.unwrap_or(pos);
                    if self.right_moved || pos.distance(start) >= RIGHT_DRAG_MIN_PX {
                        self.right_moved = true;
                        let last = self.right_last.unwrap_or(start);
                        self.extras.push(GestureIntent::Pan {
                            delta: SpVec2::new(pos.x - last.x, pos.y - last.y),
                            center: pos,
                        });
                    }
                    self.right_last = Some(pos);
                }
                // Hover is NOT a broken tap (increment 4, D2): unpressed mouse motion is
                // dropped at the adapter, so the recogniser's "motion for an untracked
                // pointer" line can only ever mean a REAL missed Down again — which is what
                // it was added to catch. Hover affordances stay parked (look-board §8:
                // nothing hover-only), so nothing touch-first is lost by dropping it.
                if self.mouse_down {
                    self.push_pointer(0, pos, 1.0, PointerKind::Mouse, PointerPhase::Moved);
                }
            },

            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                if let Some(pos) = self.cursor_logical {
                    match state {
                        ElementState::Pressed => {
                            self.mouse_down = true;
                            self.push_pointer(0, pos, 1.0, PointerKind::Mouse, PointerPhase::Down);
                        },
                        ElementState::Released => {
                            self.mouse_down = false;
                            self.push_pointer(0, pos, 0.0, PointerKind::Mouse, PointerPhase::Up);
                        },
                    }
                }
            },

            // Right button (operator ruling 2026-10-01): press-and-hold arms the canvas pan
            // (see CursorMoved); a press that NEVER moves is the context menu, immediately on
            // release (D2): the desktop hand's long-press. Same intent the recogniser
            // concludes after a 350 ms finger — same menu, same routing, no parallel semantics.
            WindowEvent::MouseInput { button: MouseButton::Right, state, .. } => {
                if let Some(pos) = self.cursor_logical {
                    match state {
                        ElementState::Pressed => {
                            self.right_down = true;
                            self.right_start = Some(pos);
                            self.right_last = Some(pos);
                            self.right_moved = false;
                        },
                        ElementState::Released => {
                            self.right_down = false;
                            if !self.right_moved {
                                self.extras.push(GestureIntent::Context { pos });
                            }
                            self.right_start = None;
                            self.right_last = None;
                            self.right_moved = false;
                        },
                    }
                }
            },

            // The wheel (operator ruling 2026-10-01): over the CANVAS it ZOOMS about the
            // cursor — the pinch's mouse hand, riding the recogniser's own `Zoom` vocabulary
            // so the camera's clamp and anchor are the one implementation. Over a SCROLLABLE
            // PANEL (inspector, library) it stays the panel's one-finger `Pan` scroll — a
            // gesture OVER a surface belongs to that surface, and sending the wheel there as
            // a Zoom would put it in the pinch's declined-over-panel arm, killing the scroll.
            // Over the remaining chrome (rail, top bar, dock) it does nothing at all. The
            // adapter knows which hardware meant what; the surface rects are the previous
            // frame's — the same stale-by-one-frame convention every hit-test here uses.
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(pos) = self.cursor_logical {
                    let layout = self.shell.last_layout.as_ref();
                    let over_panel = layout.is_some_and(|l| {
                        l.inspector.is_some_and(|r| r.contains(pos))
                            || l.library.is_some_and(|r| r.contains(pos))
                    });
                    let over_canvas = layout.is_some_and(|l| l.canvas.contains(pos));
                    if over_panel {
                        let delta = match delta {
                            MouseScrollDelta::LineDelta(_x, y) => SpVec2::new(
                                0.0,
                                y * sparq_ui::tokens::LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32,
                            ),
                            MouseScrollDelta::PixelDelta(p) => SpVec2::new(p.x as f32, p.y as f32),
                        };
                        self.extras.push(GestureIntent::Pan { delta, center: pos });
                    } else if over_canvas {
                        let factor = match delta {
                            MouseScrollDelta::LineDelta(_x, y) => WHEEL_ZOOM_STEP.powf(y),
                            MouseScrollDelta::PixelDelta(p) => {
                                (1.0 + p.y as f32 * WHEEL_ZOOM_PER_PX).clamp(0.5, 2.0)
                            },
                        };
                        if (factor - 1.0).abs() > f32::EPSILON {
                            self.extras.push(GestureIntent::Zoom { factor, center: pos });
                        }
                    }
                }
            },

            WindowEvent::Resized(size) => {
                // NonZeroU32::new already returns None for 0, so the max(1) clamp was dead code
                // pretending to handle a case the constructor handles.
                if let (Some(painter), Some(w), Some(h)) = (
                    self.painter.as_mut(),
                    std::num::NonZeroU32::new(size.width),
                    std::num::NonZeroU32::new(size.height),
                ) {
                    painter.on_window_resized(ViewportId::ROOT, w, h);
                }
                window.request_redraw();
            },

            WindowEvent::ScaleFactorChanged { .. } => {
                // Per-monitor DPI: winit already applied the new factor to the window; egui-winit
                // picks it up through take_egui_input. Logical px stay logical px — that is the
                // point of the single-transform rule at the top of this file.
                window.request_redraw();
            },

            WindowEvent::RedrawRequested => {
                let raw = self
                    .egui_winit
                    .as_mut()
                    .map_or_else(egui::RawInput::default, |ew| ew.take_egui_input(&window));
                let now_ms = self.now_ms();
                let pts = std::mem::take(&mut self.pointers);
                let extras = std::mem::take(&mut self.extras);
                let shell = &mut self.shell;
                let full = self.ctx.run_ui(raw, |ui| {
                    shell.frame(ui, FrameInput { pointers: &pts, now_ms, extras: &extras });
                });
                let ppp = full.pixels_per_point;
                let primitives = self.ctx.tessellate(full.shapes, ppp);
                let mut textures_delta = full.textures_delta;
                let ground = Palette::for_theme(self.theme).ground_base;
                let clear = [
                    ground.r() as f32 / 255.0,
                    ground.g() as f32 / 255.0,
                    ground.b() as f32 / 255.0,
                    1.0,
                ];
                if let Some(painter) = self.painter.as_mut() {
                    painter.paint_and_update_textures(
                        ViewportId::ROOT,
                        ppp,
                        clear,
                        &primitives,
                        &mut textures_delta,
                        Vec::new(),
                        &window,
                    );
                }
                // Continuous repaint for the prototype: the shell animates its log and must show
                // gesture feedback immediately. Frame-budget measurement (the 60 fps acceptance
                // criterion) reads real vsync pacing from this loop on the device.
                window.request_redraw();
            },

            _ => {},
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(p) = self.painter.as_mut() {
            p.destroy();
        }
    }
}

/// Open the shell window. Returns a process exit code.
#[must_use]
pub fn run(opts: &UiOptions) -> i32 {
    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("sparq ui: could not create the event loop: {e}");
            return 2;
        },
    };
    let mut app = ShellApp::new(opts);
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("sparq ui: event loop failed: {e}");
        return 2;
    }
    if let Some(fatal) = &app.fatal {
        eprintln!("sparq ui: {fatal}");
        return 2;
    }
    0
}
