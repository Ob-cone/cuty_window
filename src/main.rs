use device_query::{DeviceQuery, DeviceState};
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use rand::RngExt;
use std::time::Instant;
use std::{f32::consts::TAU, sync::Arc};
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

const WIDTH: u32 = 400;
const HIGHT: u32 = 300;
const PIXEL_WIDTH: u32 = 80;
const PIXEL_HIGHT: u32 = 60;

struct App {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    timer: Instant,
    rush: Option<RushData>,
    idle: Option<IdleData>,
    grab_mode: bool,
    is_grab: bool,
    hp: i32,
    is_click: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: Default::default(),
            pixels: Default::default(),
            timer: Instant::now(),
            rush: None,
            idle: None,
            grab_mode: true,
            is_grab: false,
            hp: 100,
            is_click: false,
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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("Cuty Window")
            .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HIGHT))
            .with_resizable(false)
            //.with_decorations(false)
            .with_transparent(true)
            .with_skip_taskbar(true)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        let physical_size = window.inner_size();

        let surface =
            SurfaceTexture::new(physical_size.width, physical_size.height, window.clone());
        let pixels = PixelsBuilder::new(PIXEL_WIDTH, PIXEL_HIGHT, surface)
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
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let mut mouse_pos = MousePos::default();
        let mut delta = Instant::now().duration_since(self.timer).as_secs_f32();
        delta = delta.min(0.001);
        //println!("Delta:{:?} Frame:{:?}", delta, 1.0 / delta);
        //
        let mut color = if self.grab_mode { DARK_RED } else { DARK_BLUE };
        if let Some(_window) = self.window.clone() {
            let device_state = DeviceState::new();
            let mouse = device_state.get_mouse();
            (mouse_pos.x, mouse_pos.y) = mouse.coords;
            let button = mouse.button_pressed[1];
            if !self.is_click && button {
                self.is_click = true;
                self.hp -= 10;
                color = WHITE_RED;
            }
            if !button {
                self.is_click = false;
            }
            if let Ok(pos) = _window.outer_position() {
                //println!("Pos: {:?}", pos);
                let size = _window.inner_size();
                let (center_x, center_y) = (
                    pos.x + size.width as i32 / 2,
                    pos.y + size.height as i32 / 2,
                );
                (mouse_pos.relative_x, mouse_pos.relative_y) =
                    (mouse_pos.x - center_x, mouse_pos.y - center_y);

                let len = ((mouse_pos.relative_x * mouse_pos.relative_x
                    + mouse_pos.relative_y * mouse_pos.relative_y)
                    as f32)
                    .sqrt();
                //println!("Len:{}", len);

                if let Some(monitor) = _window.current_monitor() {
                    if self.grab_mode {
                        if len > 20.0 && !self.is_grab {
                            grab_move(self, delta, mouse_pos, &_window, len, pos);
                        }
                    } else if len < 760.0 || self.rush.is_some() {
                        self.idle = None;
                        if let Some(rush) = self.rush.clone() {
                            avoid_rush(self, delta, &_window, &rush, pos);
                        } else {
                            avoid(
                                self, delta, mouse_pos, center_x, center_y, monitor, len, &_window,
                                pos,
                            );
                        }
                    } else {
                        if let Some(idle) = self.idle.clone() {
                            idle_move(
                                self, delta, &_window, &idle, center_x, center_y, monitor, pos,
                            );
                            //println!("{:?}", self.idle);
                        } else {
                            let mut rng = rand::rng();
                            let angle = rng.random_range(0.0..TAU);
                            let (x, y) = (angle.cos(), angle.sin());

                            self.idle = Some(IdleData {
                                time: Instant::now(),
                                x,
                                y,
                                change: false,
                            });
                        }
                    }
                }
            }
        }

        if let Some(pixels) = &mut self.pixels {
            for (i, pixel) in pixels.frame_mut().chunks_exact_mut(4).enumerate() {
                let x = (i % PIXEL_WIDTH as usize) as i16;
                let y = (i / PIXEL_WIDTH as usize) as i16;
                let (nomal_x, nomal_y) = normal(mouse_pos.relative_x, mouse_pos.relative_y);
                let (center_x, center_y) = ((PIXEL_WIDTH / 2) as i16, (PIXEL_HIGHT / 2) as i16);
                let (eye_center_x, eye_center_y) = (
                    (center_x as f32 + nomal_x * 5.0) as i16,
                    (center_y as f32 + nomal_y * 5.0) as i16,
                );
                let draw = draw_circle(pixel, color, x, y, eye_center_x - 20, eye_center_y, 5)
                    || draw_circle(pixel, color, x, y, eye_center_x + 20, eye_center_y, 5)
                    || draw_box(pixel, color, x, y, center_x - 30, center_y - 12, 20, 4)
                    || draw_box(pixel, color, x, y, center_x + 10, center_y - 12, 20, 4);

                if !draw {
                    pixel.copy_from_slice(BACKGROUND);
                }
            }
            let _ = pixels.render();
        }

        self.timer = Instant::now()
    }
}

fn grab_move(
    _app: &mut App,
    delta: f32,
    mouse_pos: MousePos,
    window: &Arc<Window>,
    _len: f32,
    pos: PhysicalPosition<i32>,
) {
    let (move_x, move_y) = (mouse_pos.relative_x, mouse_pos.relative_y);
    let (x, y) = normal(move_x, move_y);

    let speed = 48000.0 * delta;
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
) {
    let speed = 6000.0 * delta;
    window.set_outer_position(PhysicalPosition::new(
        pos.x as f32 + idle.x * speed,
        pos.y as f32 + idle.y * speed,
    ));
    let dur = Instant::now().duration_since(idle.time).as_secs_f32();
    if idle.change == false && (center_x < -200 || center_x > monitor.size().width as i32 + 200) {
        if let Some(_idle) = &mut app.idle {
            _idle.x *= -1.0;
            _idle.change = true;
        }
    }
    if idle.change == false && (center_y < -200 || center_y > monitor.size().height as i32 + 200) {
        if let Some(_idle) = &mut app.idle {
            _idle.y *= -1.0;
            _idle.change = true;
        }
    }
    if dur > 2.0 {
        app.idle = None;
    }
}

fn avoid_rush(
    app: &mut App,
    delta: f32,
    window: &Arc<Window>,
    rush: &RushData,
    pos: PhysicalPosition<i32>,
) {
    let speed = 36000.0 * delta;
    window.set_outer_position(PhysicalPosition::new(
        pos.x as f32 + rush.x * speed,
        pos.y as f32 + rush.y * speed,
    ));
    let dur = Instant::now().duration_since(rush.time).as_secs_f32();
    if dur > 0.25 {
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
    window: &Arc<Window>,
    pos: PhysicalPosition<i32>,
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
            x: x,
            y: y,
        });
    }
    let (x, y) = normal(move_x, move_y);

    let speed = escape_speed(len, 750.0, 48000.0) * delta;
    window.set_outer_position(PhysicalPosition::new(
        pos.x as f32 - x * speed,
        pos.y as f32 - y * speed,
    ));
}

fn normal(x: i32, y: i32) -> (f32, f32) {
    let pow_len = x * x + y * y;
    let len = (pow_len as f32).sqrt();

    if len < 0.001 {
        return (0.0, 0.0);
    }

    (x as f32 / len, y as f32 / len)
}

fn escape_speed(distance: f32, max_distance: f32, max_speed: f32) -> f32 {
    let t = 1.0 - (distance / max_distance).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t); // smoothstep
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

fn main() {
    let event_loop = EventLoop::new().expect("faile: EventLoop spaw");

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
