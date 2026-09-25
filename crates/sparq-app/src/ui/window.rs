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
use sparq_ui::pointer::{PointerEvent, PointerKind, PointerPhase};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Force, MouseButton, TouchPhase as WTouchPhase, WindowEvent};
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
    start: Instant,
    theme: ThemeChoice,
    /// Set when wgpu failed to initialise; the loop then exits with an explanation instead of
    /// silently drawing nothing (an empty window on stage is a failure that looks like a hang).
    fatal: Option<String>,
}

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
                if ke.state == ElementState::Pressed
                    && ke.physical_key == PhysicalKey::Code(KeyCode::Escape)
                {
                    event_loop.exit();
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
                let (phase, pressure) = if self.mouse_down {
                    (PointerPhase::Moved, 1.0)
                } else {
                    (PointerPhase::Moved, 0.0)
                };
                // id 0 is the mouse's stable identity (pointer.rs §2).
                self.push_pointer(0, pos, pressure, PointerKind::Mouse, phase);
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
                let shell = &mut self.shell;
                let full = self.ctx.run_ui(raw, |ui| {
                    shell.frame(ui, FrameInput { pointers: &pts, now_ms });
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
