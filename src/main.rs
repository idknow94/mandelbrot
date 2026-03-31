use pixels::{Pixels, SurfaceTexture};
use rayon::prelude::*;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{MouseScrollDelta, WindowEvent},
    event_loop::EventLoop,
    keyboard::KeyCode,
    window::{Window, WindowAttributes},
};

const ASPECT_RATIO: f64 = 1.0 / 1.0;
const LOGICAL_WIDTH: u32 = 500;
const LOGICAL_HEIGHT: u32 = (LOGICAL_WIDTH as f64 * ASPECT_RATIO) as u32;
const WIDTH: u32 = 1600;
const HEIGHT: u32 = (WIDTH as f64 * ASPECT_RATIO) as u32;
const FPS: u64 = 60;
const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / FPS);
const LN2_INV: f64 = 1.0 / std::f64::consts::LN_2;

struct Input {
    last_mouse_pos: (f64, f64),
    is_clicked: bool,
}

struct Camera {
    center_re: f64,
    center_im: f64,
    zoom: f64,
}

impl Camera {
    fn map(&self, x: u32, y: u32, width: u32, height: u32) -> (f64, f64) {
        let aspect_ratio = width as f64 / height as f64;
        let range = 4.0 / self.zoom;

        let re = self.center_re + (x as f64 / width as f64 - 0.5) * range * aspect_ratio;
        let im = self.center_im + (y as f64 / height as f64 - 0.5) * range;

        (re, im)
    }

    fn current_depth(&self) -> u32 {
        (100.0 * self.zoom.log2().max(1.0)) as u32
    }
}

struct Mandelbrot {
    window: Option<&'static Window>,
    pixels: Option<Pixels<'static>>,
    last_frame: Instant,
    last_input_time: Instant,
    camera: Camera,
    input: Input,
    lwidth: u32,
    lheight: u32,
    width: u32,
    height: u32,
}

impl Mandelbrot {
    fn get_color(i: u32, z_re: f64, z_im: f64) -> [u8; 4] {
        let mag_sq = z_re * z_re + z_im * z_im;
        let smooth_i = i as f64 + 1.0 - (mag_sq.ln() * 0.5).ln() * LN2_INV;

        let t = 0.05 * smooth_i;

        let r = (u8::MAX as f64 * (t * 0.3).sin().abs()) as u8;
        let g = (u8::MAX as f64 * (t * 0.2).sin().powi(2)) as u8;
        let b = (u8::MAX as f64 * (t * 0.7).sin().powi(4)) as u8;

        [r, g, b, 255]
    }

    fn set_resolution(&mut self, factor: u32) {
        if let Some(window) = self.window {
            let size = window.inner_size();
            let new_w = (size.width / 4) / factor;
            let new_h = (size.height / 4) / factor;

            if self.lwidth != new_w || self.lheight != new_h {
                self.lwidth = new_w;
                self.lheight = new_h;
                if let Some(pixels) = &mut self.pixels {
                    let _ = pixels.resize_buffer(new_w, new_h);
                }
            }
        }
    }
}

impl Default for Mandelbrot {
    fn default() -> Self {
        Mandelbrot {
            window: None,
            pixels: None,
            last_frame: Instant::now(),
            last_input_time: Instant::now(),
            camera: Camera {
                center_re: 0.0,
                center_im: 0.0,
                zoom: 1.0,
            },
            input: Input {
                last_mouse_pos: (0.0, 0.0),
                is_clicked: false,
            },
            lwidth: LOGICAL_WIDTH,
            lheight: LOGICAL_HEIGHT,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl ApplicationHandler for Mandelbrot {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let win_attr = WindowAttributes::default()
            .with_title("Mandelbrot Explorer")
            .with_inner_size(PhysicalSize::new(self.width, self.height));

        let win: &'static Window = Box::leak(Box::new(event_loop.create_window(win_attr).unwrap()));
        let surface_texture = SurfaceTexture::new(self.lwidth, self.lheight, win);
        let pixels = Pixels::new(self.lwidth, self.lheight, surface_texture).unwrap();

        self.window = Some(win);
        self.pixels = Some(pixels);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    match event.physical_key {
                        winit::keyboard::PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseInput { state, .. } => {
                self.input.is_clicked = state.is_pressed();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(pos) => pos.y,
                };

                let (mx, my) = self.input.last_mouse_pos;
                let (mouse_re, mouse_im) =
                    self.camera
                        .map(mx as u32, my as u32, self.width, self.height);

                if scroll > 0.0 {
                    self.camera.zoom *= 1.1;
                } else {
                    self.camera.zoom /= 1.1;
                }
                let (new_re, new_im) =
                    self.camera
                        .map(mx as u32, my as u32, self.width, self.height);
                self.camera.center_re += mouse_re - new_re;
                self.camera.center_im += mouse_im - new_im;

                self.last_input_time = Instant::now();
                self.set_resolution(2);
                if let Some(window) = self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let d_x = position.x - self.input.last_mouse_pos.0;
                let d_y = position.y - self.input.last_mouse_pos.1;

                if self.input.is_clicked {
                    let range = 4.0 / self.camera.zoom;
                    let aspect = self.width as f64 / self.height as f64;
                    self.camera.center_re -= (d_x / self.width as f64) * range * aspect;
                    self.camera.center_im -= (d_y / self.height as f64) * range;

                    self.last_input_time = Instant::now();
                    self.set_resolution(2);
                }

                self.input.last_mouse_pos = (position.x, position.y);
                if let Some(window) = self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(pixels) = &mut self.pixels {
                    let frame = pixels.frame_mut();
                    let lw = self.lwidth;
                    let lh = self.lheight;
                    let dynamic_depth = self.camera.current_depth();

                    frame
                        .par_chunks_exact_mut(4)
                        .enumerate()
                        .for_each(|(i, pixel)| {
                            let x = (i as u32) % lw;
                            let y = (i as u32) / lw;
                            let (c_re, c_im) = self.camera.map(x, y, lw, lh);

                            let mut z_re = 0.0;
                            let mut z_im = 0.0;
                            let mut color = [0, 0, 0, 255];

                            for iteration in 0..dynamic_depth {
                                let re_sq = z_re * z_re;
                                let im_sq = z_im * z_im;

                                if re_sq + im_sq > 4.0 {
                                    color = Self::get_color(iteration, z_re, z_im);
                                    break;
                                }
                                z_im = 2.0 * z_re * z_im + c_im;
                                z_re = re_sq - im_sq + c_re;
                            }
                            pixel.copy_from_slice(&color);
                        });

                    if let Err(err) = pixels.render() {
                        eprintln!("Render error: {err}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                self.width = width;
                self.height = height;
                self.lwidth = width / 2;
                self.lheight = height / 2;
                if let (Some(pixels), Some(window)) = (&mut self.pixels, &self.window) {
                    let _ = pixels.resize_buffer(self.lwidth, self.lheight);
                    let _ = pixels.resize_surface(width, height);
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let now = Instant::now();
        if now.duration_since(self.last_input_time) > Duration::from_millis(100) {
            if self.lwidth != LOGICAL_WIDTH {
                self.set_resolution(1);
                if let Some(window) = self.window {
                    window.request_redraw();
                }
            }
        }

        if now - self.last_frame >= FRAME_DURATION {
            self.last_frame = now;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = Mandelbrot::default();
    let _ = event_loop.run_app(&mut app);
}
