use std::ffi::{c_char, CString};
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use sdl2::event::Event;
use sdl2::EventPump;
use sdl2::image::{InitFlag, LoadSurface, LoadTexture};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, WindowCanvas};
use sdl2::surface::Surface;
use sdl2::sys::SDL_WindowFlags::SDL_WINDOW_SHOWN;
use sdl2::ttf::Font;

mod virtual_cam;

struct SharedData {
    last_frame: SystemTime,
    current_velocity: f64,
    current_max_velocity: f64,
    is_speaking: bool,
    speech_timings: Vec<SpeechTiming<'static>>,
    requires_update: bool,
    should_hover: bool,
    should_open_props: bool,
    pngtuber_canvas: *mut Canvas<Surface<'static>>
}

struct SpeechTiming<'a> {
    threshold: f64,
    attack_time: f64,
    release_time: f64,
    texture_surface: Surface<'a>,
    texture: Texture,
    max_velocity: f64,
}

fn str_to_c(text: &str) -> *const c_char {
    return CString::new(text).unwrap().as_c_str().as_ptr();
}

fn c_to_str(text: *mut c_char) -> *const str {
    return unsafe { CString::from_raw(text) }.as_c_str().to_str().unwrap();
}

fn create_missing_tex() -> Surface<'static> {
    let mut missing_tex = Surface::new(256, 256, PixelFormatEnum::ARGB32).unwrap();

    (*missing_tex).fill_rect(Rect::new(0, 0, 128, 128), Color::RGB(243, 60, 241)).unwrap();
    (*missing_tex).fill_rect(Rect::new(128, 0, 128, 128), Color::RGB(0, 0, 0)).unwrap();
    (*missing_tex).fill_rect(Rect::new(0, 128, 128, 128), Color::RGB(0, 0, 0)).unwrap();
    (*missing_tex).fill_rect(Rect::new(128, 128, 128, 128), Color::RGB(243, 60, 241)).unwrap();

    return missing_tex;
}

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();

    let font = ttf_context.load_font("C:/Windows/Fonts/ARIALN.TTF", 16).unwrap();

    let mut window = video_subsystem.window("Generic Title", 512, 512)
        .position_centered()
        .set_window_flags(SDL_WINDOW_SHOWN as u32)
        .build()
        .unwrap();

    let png_context = sdl2::image::init(InitFlag::all()).unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let last_frame = SystemTime::now();

    let pngtuber_surface = Surface::new(512, 512, PixelFormatEnum::ARGB32).unwrap();
    let mut pngtuber_canvas = Canvas::from_surface(pngtuber_surface).unwrap();

    let mut data = SharedData {
        last_frame,
        current_velocity: 0.0,
        is_speaking: false,
        speech_timings: Vec::new(),
        requires_update: true,
        should_hover: false,
        should_open_props: false,
        current_max_velocity: 0.0,
        pngtuber_canvas: &mut pngtuber_canvas
    };

    let png_surface = Surface::from_file("normal.png").unwrap_or(create_missing_tex());
    let png_texture = pngtuber_canvas.create_texture_from_surface(&png_surface).unwrap();

    data.speech_timings.insert(0, SpeechTiming {
        threshold: 0.0,
        attack_time: 0.0,
        release_time: 0.0,
        texture_surface: png_surface,
        texture: png_texture,
        max_velocity: 12.0
    });

    'running: loop {
        if !render(&mut canvas, &mut event_pump, &font, &mut data) {
            break 'running;
        }
    }
}

fn is_over_button(window_width: i32, x: i32, y: i32) -> bool {
    return x > (window_width - 32) && x < window_width && y < 32 && y > 0;
}

unsafe fn render_pngtuber(window_size: (u32, u32), data: &mut SharedData) {
    let canvas = (&data).pngtuber_canvas;
    let timing = (&data).speech_timings.first().unwrap();
    let surface = &timing.texture_surface;
    let tex = &timing.texture;

    let width = (&surface).width();
    let height = (&surface).height();

    let height_percent = ((window_size.1 - 24) as f64) / (height as f64);
    let new_width = ((width as f64) * height_percent) as u32;
    (*canvas).copy(&tex, None, Option::from(Rect::new(((window_size.0 / 2) - new_width / 2) as i32, 0, new_width, height))).unwrap();
}

fn render(canvas: &mut WindowCanvas, event_pump: &mut EventPump, font: &Font, data: &mut SharedData) -> bool {
    let refresh_rate = 60;

    for event in event_pump.poll_iter() {
        match event {
            Event::AppTerminating { .. } | Event::Quit { .. } => {
                return false;
            }

            Event::MouseButtonDown { mouse_btn, x, y, .. } => {

            }

            Event::MouseMotion { x, y, .. } => {
                let window_size = canvas.window().size();
                let is_over = is_over_button(window_size.0 as i32, x, y);

                if !(&data).should_hover && is_over {
                    data.should_hover = true;
                    data.requires_update = true;
                } else if (&data).should_hover && !is_over {
                    data.should_hover = false;
                    data.requires_update = true;
                }
            }

            _ => {}
        }
    }

    // Skip rendering, for performance reasons
    if !(&data).requires_update {
        sleep(Duration::new(0, 1_000_000_000u32 / refresh_rate));
        return true;
    }

    data.requires_update = false;

    let current_frame = SystemTime::now();
    let last_frame_time = SystemTime::now().duration_since((&data).last_frame).unwrap();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    unsafe {
        (*(&data).pngtuber_canvas).set_draw_color(Color::RGB(0, 0, 0));
        (*(&data).pngtuber_canvas).clear();
    }

    let window_size = canvas.window().size();

    // Delegated to a separate func, for organization purposes
    unsafe {
        render_pngtuber(window_size, data);
    }

    let pngtuber_tex = canvas.create_texture_from_surface(unsafe { (*(&data).pngtuber_canvas).surface() }).unwrap();

    canvas.copy(&pngtuber_tex, None, None).unwrap();

    // Render total FPS, not actually needed
    let text = (&font).render(&*format!("{} FPS", (1f32 / ((last_frame_time.as_millis() as f32) / 1000.0)) as u32))
        .solid(Color::WHITE)
        .unwrap();

    let text_tex = canvas.create_texture_from_surface(&text).unwrap();

    canvas.copy(&text_tex, None, Option::from(Rect::new(0, 0, text.width(), text.height()))).unwrap();

    // Render settings button
    if (&data).should_hover {
        canvas.set_draw_color(Color::RGB(175, 175, 175));
    } else {
        canvas.set_draw_color(Color::RGB(100, 100, 100));
    }

    for i in 0..3 {
        let rect = Rect::new((window_size.0 - 32) as i32, 12 + (i * 6), 24, 4);

        if (&data).should_hover {
            canvas.fill_rect(rect).unwrap();
        } else {
            canvas.draw_rect(rect).unwrap();
        }
    }

    // Free some memory
    drop((&text).context());
    unsafe {
        text_tex.destroy();
        pngtuber_tex.destroy();
    }

    canvas.present();

    data.last_frame = current_frame;

    // "VSync"
    sleep(Duration::new(0, 1_000_000_000u32 / refresh_rate));

    return true;
}