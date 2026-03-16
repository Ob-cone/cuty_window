#![windows_subsystem = "windows"]

use device_query::{DeviceQuery, DeviceState};
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use rand::RngExt;
use std::time::Instant;
use std::{f32::consts::TAU, sync::Arc};
use winapi::shared::minwindef::TRUE;
use winapi::um::dwmapi::{DWM_BLURBEHIND, DwmEnableBlurBehindWindow};
use winapi::um::winuser::SetCursorPos;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Icon;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    monitor::MonitorHandle,
    platform::windows::WindowAttributesExtWindows,
    window::{Window, WindowId},
};

const BACKGROUND: &[u8] = &[0xB8, 0xB8, 0xB8, 0x80];
const DARK_BLUE: &[u8] = &[0x06, 0x00, 0x77, 0xFF];
const DARK_RED: &[u8] = &[0x77, 0x00, 0x06, 0xFF];
const WHITE_RED: &[u8] = &[0xff, 0xf0, 0xf0, 0xFF];
const PINK: &[u8] = &[0xFF, 0x30, 0x60, 0xFF];

const HIGHT: u32 = 200;
const PIXEL_WIDTH: u32 = 80;
const PIXEL_HIGHT: u32 = 60;

const RUSH_DURATION: f32 = 0.3;
const IDLE_DURATION: f32 = 3.14;

const BASE_PIXEL_SCALE: f32 = 4.0;
const BASE_SCALE_FACTOR: f32 = 2.0;

const BASE_GRAB_THRESHOLD: f32 = 20.0;
const BASE_AVOID_THRESHOLD: f32 = 600.0;
const BASE_GRAB_SPEED: f32 = 6000.0;
const BASE_AVOID_SPEED: f32 = 6000.0;
const BASE_RUSH_SPEED: f32 = 5000.0;
const BASE_IDLE_SPEED: f32 = 1000.0;

const MAX_CLICK_HP: f32 = 100.0;
const CLICK_HP_RECOVERY: f32 = 25.0;
const EXIT_HP_RECOVERY: f32 = 30.0;
const CHANGE_TIME: u64 = 60;

const IDLE_INWARD_PROB: f64 = 0.75;

struct App {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    timer: Instant,
    mode_timer: Instant,
    is_cursor_on_window: bool,
    rush: Option<RushData>,
    idle: Option<IdleData>,
    grab_mode: bool,
    is_grab: bool,
    click_hp: f32,
    exit_hp: f32,
    is_click: bool,
    pixel_scale: f32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: Default::default(),
            pixels: Default::default(),
            timer: Instant::now(),
            mode_timer: Instant::now(),
            is_cursor_on_window: false,
            rush: None,
            idle: None,
            grab_mode: false,
            is_grab: false,
            click_hp: MAX_CLICK_HP,
            exit_hp: PIXEL_HIGHT as f32,
            is_click: false,
            pixel_scale: BASE_PIXEL_SCALE,
        }
    }
}

#[derive(Clone, Copy)]
struct RushData {
    time: Instant,
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug)]
struct IdleData {
    time: Instant,
    x: f32,
    y: f32,
    change: bool,
}

#[derive(Default, Debug, Clone, Copy)]
struct MousePos {
    x: i32,
    y: i32,
    relative_x: i32,
    relative_y: i32,
}

fn scaled(base: f32, pixel_scale: f32) -> f32 {
    base * (pixel_scale / BASE_PIXEL_SCALE)
}

fn scaled_threshold(base: f32, scale_factor: f32) -> f32 {
    base * (scale_factor / BASE_SCALE_FACTOR)
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next());

        let logical_scale = monitor
            .as_ref()
            .map(|mon| {
                let logical_height = mon.size().height as f64 / mon.scale_factor();
                (logical_height / 1080.0).clamp(0.5, 3.0) as f32
            })
            .unwrap_or(1.0);

        let pixel_scale = ((HIGHT as f32 * logical_scale) / PIXEL_HIGHT as f32).max(1.0);

        let win_width = (PIXEL_WIDTH as f32 * pixel_scale).round() as u32;
        let win_height = (PIXEL_HIGHT as f32 * pixel_scale).round() as u32;

        self.pixel_scale = pixel_scale;

        println!(
            "win: {:?}, pixel_scale: {}",
            (win_width, win_height),
            pixel_scale
        );

        let base_attrs = Window::default_attributes()
            .with_title("Cuty Window")
            .with_window_icon(Some(load_icon()))
            .with_inner_size(winit::dpi::LogicalSize::new(win_width, win_height))
            .with_resizable(false)
            .with_transparent(true)
            .with_skip_taskbar(true)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop);

        let window_attributes = if is_windows_11() {
            println!("Win11");
            base_attrs
        } else {
            println!("Win10");
            base_attrs.with_no_redirection_bitmap(true).with_blur(true)
        };

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        if !is_windows_11() {
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Win32(win32) = handle.as_raw() {
                    unsafe {
                        let hwnd = win32.hwnd.get() as winapi::shared::windef::HWND;
                        let bb = DWM_BLURBEHIND {
                            dwFlags: 0x01,
                            fEnable: TRUE,
                            hRgnBlur: std::ptr::null_mut(),
                            fTransitionOnMaximized: 0,
                        };
                        DwmEnableBlurBehindWindow(hwnd, &bb);
                    }
                }
            }
        }

        let physical_size = window.inner_size();
        let surface =
            SurfaceTexture::new(physical_size.width, physical_size.height, window.clone());
        let pixels = PixelsBuilder::new(win_width, win_height, surface)
            .surface_texture_format(pixels::wgpu::TextureFormat::Rgba8UnormSrgb)
            .blend_state(pixels::wgpu::BlendState::ALPHA_BLENDING)
            .clear_color(pixels::wgpu::Color::TRANSPARENT)
            .build()
            .unwrap();
        self.pixels = Some(pixels);

        self.window = Some(window);
        self.timer = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                use device_query::{DeviceQuery, DeviceState, Keycode};
                let device_state = DeviceState::new();
                let keys = device_state.get_keys();
                let is_alt_f4 = keys.contains(&Keycode::LAlt) || keys.contains(&Keycode::RAlt);
                if is_alt_f4 {
                    if self.is_cursor_on_window && self.grab_mode {
                        self.exit_hp -= 5.0;
                        println!("Exit: {:?}", self.exit_hp);
                    }
                    if self.exit_hp <= 0.0 {
                        event_loop.exit();
                    }
                } else {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let delta = Instant::now().duration_since(self.timer).as_secs_f32();

        if delta < 1.0 / 60.0 {
            return;
        }
        let delta = delta.min(1.0 / 20.0);

        let mut mouse_pos = MousePos::default();

        let ps = self.pixel_scale;

        let mut color = if self.grab_mode { DARK_RED } else { DARK_BLUE };

        let mode_dur = Instant::now().duration_since(self.mode_timer).as_secs();
        if !self.grab_mode && mode_dur > CHANGE_TIME {
            let mut rng = rand::rng();
            let is_change = rng.random_bool(0.5);
            if is_change {
                self.grab_mode = true;
                self.is_grab = false;
            }
            self.mode_timer = Instant::now();
        }

        if let Some(_window) = self.window.clone() {
            let device_state = DeviceState::new();
            let mouse = device_state.get_mouse();
            (mouse_pos.x, mouse_pos.y) = mouse.coords;

            self.is_cursor_on_window = if let Ok(pos) = _window.outer_position() {
                let size = _window.inner_size();
                mouse_pos.x >= pos.x
                    && mouse_pos.x <= pos.x + size.width as i32
                    && mouse_pos.y >= pos.y
                    && mouse_pos.y <= pos.y + size.height as i32
            } else {
                false
            };

            let button = mouse.button_pressed[1];
            if !self.is_click && button && self.is_cursor_on_window {
                self.is_click = true;
                self.click_hp -= 10.0;
                if self.grab_mode {
                    color = WHITE_RED;
                }
                println!("Hp: {:?}", self.click_hp);
            }
            if !button {
                self.is_click = false;
            }
            if self.click_hp < 0.0 && self.grab_mode {
                self.grab_mode = false;
                self.mode_timer = Instant::now();
                self.click_hp = MAX_CLICK_HP;
                self.exit_hp = PIXEL_HIGHT as f32;
            } else if self.click_hp < MAX_CLICK_HP {
                self.click_hp += CLICK_HP_RECOVERY * delta;
            }
            if self.exit_hp < PIXEL_WIDTH as f32 && self.grab_mode {
                self.exit_hp += EXIT_HP_RECOVERY * delta;
            }

            if let Ok(pos) = _window.outer_position() {
                let size = _window.inner_size();
                let (center_x, center_y) = (
                    pos.x + size.width as i32 / 2,
                    pos.y + size.height as i32 / 2,
                );
                (mouse_pos.relative_x, mouse_pos.relative_y) =
                    (mouse_pos.x - center_x, mouse_pos.y - center_y);

                let len = ((mouse_pos.relative_x as f32).powi(2)
                    + (mouse_pos.relative_y as f32).powi(2))
                .sqrt();

                if let Some(monitor) = _window.current_monitor() {
                    let sf = monitor.scale_factor() as f32;
                    let grab_threshold = scaled_threshold(BASE_GRAB_THRESHOLD, sf);
                    let avoid_threshold = scaled_threshold(BASE_AVOID_THRESHOLD, sf);

                    if self.grab_mode {
                        if len > grab_threshold && !self.is_grab {
                            grab_move(self, delta, mouse_pos, &_window, len, pos, ps, sf);
                        } else if self.is_grab {
                            unsafe {
                                SetCursorPos(center_x, center_y);
                            }
                            idle_or_wander(
                                self, delta, &_window, center_x, center_y, monitor, pos, ps, sf,
                                false,
                            );
                        } else {
                            self.is_grab = true;
                            self.click_hp = MAX_CLICK_HP;
                            self.exit_hp = PIXEL_HIGHT as f32;
                        }
                    } else if len < avoid_threshold || self.rush.is_some() {
                        self.idle = None;
                        if let Some(rush) = self.rush.clone() {
                            avoid_rush(self, delta, &_window, &rush, pos, ps, sf);
                        } else {
                            avoid(
                                self,
                                delta,
                                mouse_pos,
                                center_x,
                                center_y,
                                monitor,
                                len,
                                avoid_threshold,
                                &_window,
                                pos,
                                ps,
                                sf,
                            );
                        }
                    } else {
                        idle_or_wander(
                            self, delta, &_window, center_x, center_y, monitor, pos, ps, sf, true,
                        );
                    }
                }
            }
        }

        if let Some(pixels) = &mut self.pixels {
            let ps = self.pixel_scale;
            let win_width = (PIXEL_WIDTH as f32 * ps).round() as usize;
            let (nomal_x, nomal_y) = normal(mouse_pos.relative_x, mouse_pos.relative_y);
            let (center_x, center_y) = ((PIXEL_WIDTH / 2) as i16, (PIXEL_HIGHT / 2) as i16);
            let (eye_center_x, eye_center_y) = (
                (center_x as f32 + nomal_x * 5.0) as i16,
                (center_y as f32 + nomal_y * 5.0) as i16,
            );

            for (i, pixel) in pixels.frame_mut().chunks_exact_mut(4).enumerate() {
                let ox = (i % win_width) as f32;
                let oy = (i / win_width) as f32;
                let x = (ox / ps).floor() as i16;
                let y = (oy / ps).floor() as i16;

                if x >= PIXEL_WIDTH as i16 || y >= PIXEL_HIGHT as i16 {
                    pixel.copy_from_slice(BACKGROUND);
                    continue;
                }

                let draw = draw_circle(pixel, color, x, y, eye_center_x - 20, eye_center_y, 5)
                    || draw_circle(pixel, color, x, y, eye_center_x + 20, eye_center_y, 5)
                    || draw_box(pixel, color, x, y, center_x - 30, center_y - 12, 20, 4)
                    || draw_box(pixel, color, x, y, center_x + 10, center_y - 12, 20, 4);

                if !draw {
                    let background = if y as f32 > self.exit_hp {
                        PINK
                    } else {
                        BACKGROUND
                    };
                    pixel.copy_from_slice(background);
                }
            }
            let _ = pixels.render();
        }

        self.timer = Instant::now()
    }
}

fn idle_or_wander(
    app: &mut App,
    delta: f32,
    window: &Arc<Window>,
    center_x: i32,
    center_y: i32,
    monitor: MonitorHandle,
    pos: PhysicalPosition<i32>,
    pixel_scale: f32,
    scale_factor: f32,
    allow_offscreen: bool,
) {
    if let Some(idle) = app.idle.clone() {
        idle_move(
            app,
            delta,
            window,
            &idle,
            center_x,
            center_y,
            monitor,
            pos,
            pixel_scale,
            scale_factor,
            allow_offscreen,
        );
    } else {
        let mut rng = rand::rng();
        let angle = if rng.random_bool(IDLE_INWARD_PROB) {
            let mon_cx = monitor.size().width as f32 / 2.0;
            let mon_cy = monitor.size().height as f32 / 2.0;
            let dx = mon_cx - center_x as f32;
            let dy = mon_cy - center_y as f32;
            let base_angle = dy.atan2(dx);
            let spread = std::f32::consts::FRAC_PI_4;
            base_angle + rng.random_range(-spread..spread)
        } else {
            rng.random_range(0.0..TAU)
        };
        app.idle = Some(IdleData {
            time: Instant::now(),
            x: angle.cos(),
            y: angle.sin(),
            change: false,
        });
    }
}

fn grab_move(
    _app: &mut App,
    delta: f32,
    mouse_pos: MousePos,
    window: &Arc<Window>,
    _len: f32,
    pos: PhysicalPosition<i32>,
    pixel_scale: f32,
    scale_factor: f32,
) {
    let (move_x, move_y) = (mouse_pos.relative_x, mouse_pos.relative_y);
    let (x, y) = normal(move_x, move_y);
    let speed = scaled(BASE_GRAB_SPEED, pixel_scale) * (scale_factor / BASE_SCALE_FACTOR) * delta;
    window.set_outer_position(PhysicalPosition::new(
        pos.x as f32 + x * speed,
        pos.y as f32 + y * speed,
    ));
}

fn idle_move(
    app: &mut App,
    delta: f32,
    window: &Arc<Window>,
    idle: &IdleData,
    center_x: i32,
    center_y: i32,
    monitor: MonitorHandle,
    pos: PhysicalPosition<i32>,
    pixel_scale: f32,
    scale_factor: f32,
    allow_offscreen: bool,
) {
    let speed = scaled(BASE_IDLE_SPEED, pixel_scale) * (scale_factor / BASE_SCALE_FACTOR) * delta;

    let mut new_x = pos.x as f32 + idle.x * speed;
    let mut new_y = pos.y as f32 + idle.y * speed;

    if !allow_offscreen {
        let mon_w = monitor.size().width as f32;
        let mon_h = monitor.size().height as f32;
        let win_w = window.inner_size().width as f32;
        let win_h = window.inner_size().height as f32;

        new_x = new_x.clamp(0.0, (mon_w - win_w).max(0.0));
        new_y = new_y.clamp(0.0, (mon_h - win_h).max(0.0));

        if let Some(_idle) = &mut app.idle {
            if new_x <= 0.0 || new_x >= mon_w - win_w {
                _idle.x *= -1.0;
            }
            if new_y <= 0.0 || new_y >= mon_h - win_h {
                _idle.y *= -1.0;
            }
        }
    }

    window.set_outer_position(PhysicalPosition::new(new_x, new_y));

    let dur = Instant::now().duration_since(idle.time).as_secs_f32();

    if allow_offscreen {
        if idle.change == false && (center_x < -200 || center_x > monitor.size().width as i32 + 200)
        {
            if let Some(_idle) = &mut app.idle {
                _idle.x *= -1.0;
                _idle.change = true;
            }
        }
        if idle.change == false
            && (center_y < -200 || center_y > monitor.size().height as i32 + 200)
        {
            if let Some(_idle) = &mut app.idle {
                _idle.y *= -1.0;
                _idle.change = true;
            }
        }
    }

    if dur > IDLE_DURATION {
        app.idle = None;
    }
}

fn avoid_rush(
    app: &mut App,
    delta: f32,
    window: &Arc<Window>,
    rush: &RushData,
    pos: PhysicalPosition<i32>,
    pixel_scale: f32,
    scale_factor: f32,
) {
    let speed = scaled(BASE_RUSH_SPEED, pixel_scale) * (scale_factor / BASE_SCALE_FACTOR) * delta;
    window.set_outer_position(PhysicalPosition::new(
        pos.x as f32 + rush.x * speed,
        pos.y as f32 + rush.y * speed,
    ));
    let dur = Instant::now().duration_since(rush.time).as_secs_f32();
    if dur > RUSH_DURATION {
        app.rush = None;
    }
}

fn avoid(
    app: &mut App,
    delta: f32,
    mouse_pos: MousePos,
    center_x: i32,
    center_y: i32,
    monitor: MonitorHandle,
    len: f32,
    avoid_threshold: f32,
    window: &Arc<Window>,
    pos: PhysicalPosition<i32>,
    pixel_scale: f32,
    scale_factor: f32,
) {
    let (mut move_x, mut move_y) = (mouse_pos.relative_x, mouse_pos.relative_y);
    if center_x < 0 || center_x > monitor.size().width as i32 {
        move_x = 0;
    }
    if center_y < 0 || center_y > monitor.size().height as i32 {
        move_y = 0;
    }
    if move_x == 0 && move_y == 0 {
        let (x, y) = normal(mouse_pos.relative_x, mouse_pos.relative_y);
        app.rush = Some(RushData {
            time: Instant::now(),
            x,
            y,
        });
    }
    let (x, y) = normal(move_x, move_y);
    let speed = escape_speed(
        len,
        avoid_threshold,
        scaled(BASE_AVOID_SPEED, pixel_scale) * (scale_factor / BASE_SCALE_FACTOR),
    ) * delta;
    window.set_outer_position(PhysicalPosition::new(
        pos.x as f32 - x * speed,
        pos.y as f32 - y * speed,
    ));
}

fn normal(x: i32, y: i32) -> (f32, f32) {
    let len = ((x as f32).powi(2) + (y as f32).powi(2)).sqrt();
    if len < 0.001 {
        return (0.0, 0.0);
    }
    (x as f32 / len, y as f32 / len)
}

fn escape_speed(distance: f32, max_distance: f32, max_speed: f32) -> f32 {
    let t = 1.0 - (distance / max_distance).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    smooth * max_speed
}

fn draw_circle(
    pixel: &mut [u8],
    color: &[u8],
    x: i16,
    y: i16,
    point_x: i16,
    point_y: i16,
    raduis: i16,
) -> bool {
    let len = (point_x - x) * (point_x - x) + (point_y - y) * (point_y - y);
    if len < raduis * raduis {
        pixel.copy_from_slice(color);
        true
    } else {
        false
    }
}

fn draw_box(
    pixel: &mut [u8],
    color: &[u8],
    x: i16,
    y: i16,
    point_x: i16,
    point_y: i16,
    width: i16,
    height: i16,
) -> bool {
    if (point_x < x) && (x < point_x + width) && (point_y < y) && (y < point_y + height) {
        pixel.copy_from_slice(color);
        true
    } else {
        false
    }
}

fn is_windows_11() -> bool {
    use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryW};
    use winapi::um::winnt::OSVERSIONINFOW;

    type RtlGetVersionFn = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32;

    unsafe {
        let ntdll: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
        let module = LoadLibraryW(ntdll.as_ptr());
        if module.is_null() {
            return false;
        }
        let func = GetProcAddress(module, b"RtlGetVersion\0".as_ptr() as _);
        if func.is_null() {
            return false;
        }
        let rtl_get_version: RtlGetVersionFn = std::mem::transmute(func);
        let mut info: OSVERSIONINFOW = std::mem::zeroed();
        info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
        rtl_get_version(&mut info);
        info.dwBuildNumber >= 22000
    }
}

fn load_icon() -> Icon {
    let bytes = include_bytes!("../Cuty_window.png");

    let image = image::load_from_memory(bytes)
        .expect("fail: icon load")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();

    Icon::from_rgba(rgba, width, height).expect("fail: Icon spawn")
}

fn main() {
    let event_loop = EventLoop::new().expect("faile: EventLoop spaw");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
