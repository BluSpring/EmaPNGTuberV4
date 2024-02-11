use std::ffi::{c_char, c_void, CString};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::mem::size_of;
use std::ops::Index;
use std::ptr::null_mut;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use close_file::Closable;
use cpal::{Device, Host};
use cpal::traits::{DeviceTrait, HostTrait};
use imgui::{Condition, Context, DrawCmd, TreeNodeFlags, Ui};
use imgui::internal::{RawCast, RawWrapper};
use mint::Vector2;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use rfd::FileDialog;
use sdl2::event::Event;
use sdl2::EventPump;
use sdl2::image::{InitFlag, LoadSurface, LoadTexture};
use sdl2::libc::{c_int, free, malloc, size_t};
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture, TextureAccess, WindowCanvas};
use sdl2::surface::Surface;
use sdl2::sys::SDL_WindowFlags::SDL_WINDOW_SHOWN;
use sdl2::ttf::Font;
use sdl2::video::GLProfile;
use sdl2_sys::{SDL_BlendFactor, SDL_BlendOperation, SDL_Color, SDL_ComposeCustomBlendMode, SDL_DestroyTexture, SDL_FPoint, SDL_RenderGeometry, SDL_SetRenderDrawBlendMode, SDL_Texture, SDL_Vertex};
use serde::{Deserialize, Serialize};
use serde::de::Error;
use winsafe::{COLORREF, HWND};
use winsafe::co::{GWLP, LWA, WS_EX};
use winsafe::prelude::*;

use crate::imgui_support::SdlPlatform;

mod imgui_support;

struct SharedData {
    last_frame: SystemTime,
    current_velocity: f64,
    current_max_velocity: f64,
    is_speaking: bool,
    speech_timings: *mut Vec<SpeechTiming<'static>>,
    requires_update: bool,
    should_hover: bool,
    should_open_props: bool,
    pngtuber_canvas: *mut Canvas<Surface<'static>>,
    should_render_props: bool,
    is_props_open: bool,
    imgui: *mut Context,
    imgui_platform: *mut SdlPlatform,
    imgui_fonts_texture: *mut Texture,
    is_bordered: bool,
    input_device_name: String,
    input_device_index: usize,
    input_devices: *mut Vec<Device>,
    current_max_frames: i32,
    current_frame: i32,
    host: Host,
    input_device: Option<Device>
}

#[derive(Serialize, Deserialize, Debug)]
struct SavedData {
    input_device: String,
    speech_timings: Vec<SavedSpeechData>
}

#[derive(Serialize, Deserialize, Debug)]
struct SavedSpeechData {
    threshold: f32,
    attack_time: f32,
    release_time: f32,
    texture_path: String,

    should_bounce: bool,
    max_velocity: f32,
    total_velocity_frames: i32
}

struct SpeechTiming<'a> {
    threshold: f32,
    attack_time: f32,
    release_time: f32,
    texture_path: String,
    texture_surface: Surface<'a>,
    texture: Texture,
    max_velocity: f32,
    should_bounce: bool,
    total_velocity_frames: i32,
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

    missing_tex
}

fn save(shared_data: &mut SharedData) {
    let mut saved_data = SavedData {
        input_device: shared_data.input_device_name.clone(),
        speech_timings: Vec::new()
    };

    for (i, timing) in unsafe { (*shared_data.speech_timings).iter().clone() }.enumerate() {
        let speech_timing = SavedSpeechData {
            threshold: timing.threshold,
            attack_time: timing.attack_time,
            release_time: timing.release_time,
            texture_path: timing.texture_path.clone(),

            should_bounce: timing.should_bounce,
            max_velocity: timing.max_velocity,
            total_velocity_frames: timing.total_velocity_frames
        };

        saved_data.speech_timings.insert(i, speech_timing);
    }

    let serialized = serde_yaml::to_string(&saved_data);

    if serialized.is_err() {
        eprintln!("Error occurred while serializing: {}", serialized.unwrap_err().to_string().as_str());
        return;
    }

    let mut file = OpenOptions::new().write(true).open("pngtuber_data.yml").unwrap_or(File::create("pngtuber_data.yml").unwrap());
    file.write_all(serialized.unwrap().as_bytes()).unwrap();
    file.close().unwrap();
}

fn load(shared_data: &mut SharedData) {
    let file = File::open("pngtuber_data.yml");

    if file.is_err() {
        eprintln!("Error occurred while deserializing: {}", file.unwrap_err().to_string().as_str());
        return;
    }

    let mut contents = String::new();
    let mut file_thing = file.unwrap();
    file_thing.read_to_string(&mut contents).unwrap();

    let saved_data_opt: Result<SavedData, serde_yaml::Error> = serde_yaml::from_str(contents.as_str());

    if saved_data_opt.is_err() {
        eprintln!("Failed to deserialize data, reverting to defaults! Error: {}", saved_data_opt.unwrap_err().to_string().as_str());
        file_thing.close().unwrap();
        return;
    }

    let saved_data: SavedData = saved_data_opt.unwrap();

    shared_data.input_device_name = saved_data.input_device;
    for (i, timing) in saved_data.speech_timings.iter().enumerate() {
        let texture_path = timing.texture_path.clone();
        let png_surface = Surface::from_file(texture_path).unwrap_or(create_missing_tex());
        // i thought this was already in unsafe but okay
        let png_texture = unsafe { (*shared_data.pngtuber_canvas).create_texture_from_surface(&png_surface).unwrap() };

        let speech_timing = SpeechTiming {
            threshold: timing.threshold,
            attack_time: timing.attack_time,
            release_time: timing.release_time,

            texture_path: timing.texture_path.clone(), // thanks rust.
            texture_surface: png_surface,
            texture: png_texture,

            should_bounce: timing.should_bounce,
            max_velocity: timing.max_velocity,
            total_velocity_frames: timing.total_velocity_frames
        };

        let timings = shared_data.speech_timings;
        unsafe {
            (*timings).insert(i, speech_timing);
        }
    }

    file_thing.close().unwrap();
}

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();

    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_version(3, 3);
    gl_attr.set_context_profile(GLProfile::Core);

    let font = ttf_context.load_font("C:/Windows/Fonts/ARIALN.TTF", 16).unwrap();

    let window = video_subsystem.window("Generic Title", 512, 512)
        .position_centered()
        .set_window_flags(SDL_WINDOW_SHOWN as u32)
        .borderless()
        .opengl()
        .build()
        .unwrap();

    let png_context = sdl2::image::init(InitFlag::all()).unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGBA(0, 0, 0, 0));
    canvas.clear();

    unsafe {
        if let RawWindowHandle::Win32(handle) = canvas.window().raw_window_handle() {
            let hwnd: HWND = Handle::from_ptr(handle.hwnd);

            hwnd.SetWindowLongPtr(GWLP::EXSTYLE, hwnd.GetWindowLongPtr(GWLP::EXSTYLE) | (WS_EX::LAYERED.raw() as isize));
            hwnd.SetLayeredWindowAttributes(COLORREF::new(14, 14, 14), 0, LWA::COLORKEY).unwrap();
        }
    }

    canvas.present();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let last_frame = SystemTime::now();

    let pngtuber_surface = Surface::new(512, 512, PixelFormatEnum::ARGB32).unwrap();
    let mut pngtuber_canvas = Canvas::from_surface(pngtuber_surface).unwrap();

    let gl_context = canvas.window().gl_create_context().unwrap();
    canvas.window().gl_make_current(&gl_context).unwrap();

    canvas.window().subsystem().gl_set_swap_interval(1).unwrap();

    let mut imgui = Context::create();

    imgui.set_ini_filename(None);
    imgui.set_log_filename(None);

    let mut platform = SdlPlatform::init(&mut imgui);

    let mut data = SharedData {
        last_frame,
        current_velocity: 0.0,
        current_max_velocity: 0.0,
        current_frame: 0,
        current_max_frames: 0,
        is_speaking: false,
        speech_timings: &mut Vec::new(),
        requires_update: true,
        should_render_props: false,
        should_hover: false,
        should_open_props: false,
        pngtuber_canvas: &mut pngtuber_canvas,
        is_props_open: false,
        imgui: &mut imgui,
        imgui_platform: &mut platform,
        imgui_fonts_texture: null_mut(),
        is_bordered: false,
        input_device_name: String::from(""),
        input_device_index: 0,
        input_devices: &mut Vec::new(),
        input_device: None,
        host: cpal::default_host()
    };

    imgui
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);

    // if you're wondering what the fuck this is meant to be,
    // I HAVE NO IDEA EITHER.
    // THIS WAS LITERALLY THE ONLY WAY I COULD FIX THIS.
    unsafe {
        let imgui_fonts_texture = &mut (*data.imgui).fonts().build_rgba32_texture();

        let mut sdl_tex = canvas.texture_creator().create_texture(PixelFormatEnum::ARGB8888, TextureAccess::Static, imgui_fonts_texture.width, imgui_fonts_texture.height).unwrap();
        sdl_tex.update(None, imgui_fonts_texture.data, (4 * imgui_fonts_texture.width) as usize).unwrap();
        sdl_tex.set_blend_mode(BlendMode::Blend);

        data.imgui_fonts_texture = &mut sdl_tex;
    }

    unsafe {
        let blend_mode = SDL_ComposeCustomBlendMode(
            SDL_BlendFactor::SDL_BLENDFACTOR_SRC_ALPHA,
            SDL_BlendFactor::SDL_BLENDFACTOR_ONE_MINUS_SRC_ALPHA,
            SDL_BlendOperation::SDL_BLENDOPERATION_ADD,
            SDL_BlendFactor::SDL_BLENDFACTOR_ONE,
            SDL_BlendFactor::SDL_BLENDFACTOR_ONE_MINUS_SRC_ALPHA,
            SDL_BlendOperation::SDL_BLENDOPERATION_ADD
        );

        SDL_SetRenderDrawBlendMode(canvas.raw(), blend_mode);
    }

    load(&mut data);
    update_input_devices(&mut data);

    unsafe {
        if (*data.speech_timings).is_empty() {
            canvas.window_mut().set_bordered(true);
            data.is_bordered = true;
            data.should_open_props = true;
            data.is_props_open = true;
            data.should_render_props = true;
        }
    }

    'running: loop {
        if !render(&mut canvas, &mut event_pump, &font, &mut data) {
            break 'running;
        }
    }
}

fn update_input_devices(data: &mut SharedData) {
    unsafe {
        (*data.input_devices).clear();
    }

    for (i, input_device) in data.host.input_devices().unwrap().enumerate() {
        unsafe {
            (*data.input_devices).insert(i, input_device);
        }
    }

    if data.input_device.is_none() {
        if data.input_device_name.is_empty() {
            let default = data.host.default_input_device();
            if default.is_some() {
                let unwrapped = default.unwrap();

                data.input_device_name = unwrapped.name().unwrap();
                let _ = data.input_device.insert(unwrapped);
            }
        } else {
            unsafe {
                for x in data.host.input_devices().unwrap() {
                    if x.name().unwrap() == data.input_device_name {
                        let _ = data.input_device.insert(x);
                        break;
                    }
                }

                if data.input_device.is_none() {
                    let default = data.host.default_input_device();
                    if default.is_some() {
                        let _ = data.input_device.insert(default.unwrap());
                    }
                }
            }
        }

        unsafe {
            if data.input_device.is_some() {
                let mut i = 0;
                for x in (*data.input_devices).iter() {
                    if x.name().unwrap() == data.input_device_name {
                        data.input_device_index = i;
                        break;
                    }

                    i += 1;
                }
            }
        }
    }
}

fn create_default_timing(data: &mut SharedData) -> SpeechTiming<'static> {
    SpeechTiming {
        threshold: 0.0,
        attack_time: 0.0,
        release_time: 0.0,
        texture_surface: create_missing_tex(),
        texture: unsafe {
            (*data.pngtuber_canvas).create_texture_from_surface(create_missing_tex())
        }.unwrap(),
        max_velocity: 12.0,
        should_bounce: false,
        texture_path: String::from(""),
        total_velocity_frames: 0
    }
}

fn is_over_button(window_width: i32, x: i32, y: i32) -> bool {
    x > (window_width - 32) && x < window_width && y < 32 && y > 0
}

unsafe fn render_pngtuber(window_size: (u32, u32), data: &mut SharedData) {
    let canvas = data.pngtuber_canvas;
    if (*data.speech_timings).is_empty() {
        return;
    }

    let timing = unsafe { (*data.speech_timings).first() }.unwrap();
    let surface = &timing.texture_surface;
    let tex = &timing.texture;

    let width = surface.width();
    let height = surface.height();

    let height_percent = ((window_size.1 - 24) as f64) / (height as f64);
    let new_width = ((width as f64) * height_percent) as u32;
    (*canvas).copy(tex, None, Option::from(Rect::new(((window_size.0 / 2) - new_width / 2) as i32, 0, new_width, height))).unwrap();
}

fn render(canvas: &mut WindowCanvas, event_pump: &mut EventPump, font: &Font, data: &mut SharedData) -> bool {
    let refresh_rate = 90;

    for event in event_pump.poll_iter() {
        if data.is_props_open {
            unsafe {
                (*data.imgui_platform).handle_event(&mut *data.imgui, &event);
            }
        }

        match event {
            Event::AppTerminating { .. } | Event::Quit { .. } => {
                return false;
            }

            Event::AppDidEnterBackground { .. } |
            Event::AppWillEnterBackground { .. }
            => {
                data.requires_update = true;
            }

            Event::MouseButtonDown { mouse_btn, x, y, .. } => {
                let window_size = canvas.window().size();

                if mouse_btn == MouseButton::Left && is_over_button(window_size.0 as i32, x, y) {
                    data.is_props_open = true;
                    data.requires_update = true;
                } else if mouse_btn == MouseButton::Right {
                    let window = canvas.window_mut();
                    data.is_bordered = !data.is_bordered;
                    data.should_render_props = !data.should_render_props;
                    window.set_bordered(data.is_bordered);
                    data.requires_update = true;
                }
            }

            Event::MouseMotion { x, y, .. } => {
                let window_size = canvas.window().size();
                let is_over = is_over_button(window_size.0 as i32, x, y) && !data.is_props_open;

                if !data.should_hover && is_over {
                    data.should_hover = true;
                    data.requires_update = true;
                } else if data.should_hover && !is_over {
                    data.should_hover = false;
                    data.requires_update = true;
                }
            }

            _ => {}
        }
    }

    /*if canvas.window().has_mouse_focus() && !(&data).should_render_props {
        data.should_render_props = true;
        data.requires_update = true;
    } else if !canvas.window().has_mouse_focus() && (&data).should_render_props && !(&data).is_props_open {
        data.should_render_props = false;
        data.requires_update = true;
    }*/

    // Skip rendering, for performance reasons
    if !data.requires_update {
        sleep(Duration::new(0, 1_000_000_000u32 / refresh_rate));
        return true;
    }

    if !data.is_props_open {
        data.requires_update = false;
    }

    let current_frame = SystemTime::now();
    let last_frame_time = SystemTime::now().duration_since(data.last_frame).unwrap();

    canvas.set_draw_color(Color::RGBA(14, 14, 14, 0));
    canvas.clear();

    unsafe {
        (*data.pngtuber_canvas).set_draw_color(Color::RGBA(0, 0, 0, 0));
        (*data.pngtuber_canvas).clear();
    }

    let window_size = canvas.window().size();

    // Delegated to a separate func, for organization purposes
    unsafe {
        render_pngtuber(window_size, data);
    }

    let pngtuber_tex = canvas.create_texture_from_surface(unsafe { (*data.pngtuber_canvas).surface() }).unwrap();

    canvas.copy(&pngtuber_tex, None, None).unwrap();

    // Render total FPS, not actually needed
    let text = font.render(&format!("{} FPS", (1f32 / ((last_frame_time.as_millis() as f32) / 1000.0)) as u32))
        .solid(Color::WHITE)
        .unwrap();

    let text_tex = canvas.create_texture_from_surface(&text).unwrap();

    canvas.copy(&text_tex, None, Option::from(Rect::new(0, 0, text.width(), text.height()))).unwrap();

    // Render settings button
    if data.should_render_props {
        canvas.set_draw_color(Color::RGB(34, 34, 34));
        canvas.fill_rect(Rect::new((window_size.0 - 32) as i32, 0, 24, 24)).unwrap();

        if data.should_hover {
            canvas.set_draw_color(Color::RGB(175, 175, 175));
        } else {
            canvas.set_draw_color(Color::RGB(100, 100, 100));
        }

        for i in 0..3 {
            let rect = Rect::new((window_size.0 - 32) as i32, 12 + (i * 6), 24, 4);

            if data.should_hover {
                canvas.fill_rect(rect).unwrap();
            } else {
                canvas.draw_rect(rect).unwrap();
            }
        }
    }

    // Free some memory
    drop(text.context());
    unsafe {
        text_tex.destroy();
        pngtuber_tex.destroy();
    }

    if data.is_props_open {
        unsafe {
            let platform = data.imgui_platform;
            let imgui = data.imgui;

            (*platform).prepare_frame(&mut *imgui, canvas.window(), event_pump);

            let ui = (*imgui).new_frame();

            if !render_ui(ui, data) {
                return false;
            }

            let draw_data = (*imgui).render();

            let raw_draw_data = draw_data.raw();
            let vertices: *mut SDL_Vertex = malloc((raw_draw_data.TotalVtxCount * (size_of::<SDL_Vertex>() as i32)) as size_t) as *mut SDL_Vertex;
            let indices: *mut c_int = malloc((raw_draw_data.TotalIdxCount * (size_of::<c_int>() as i32)) as size_t) as *mut c_int;

            for list in draw_data.draw_lists() {
                for vtx_id in 0..list.vtx_buffer().len() {
                    let vtx = list.vtx_buffer().get(vtx_id).unwrap();
                    vertices.add(vtx_id).write(SDL_Vertex {
                        color: SDL_Color {
                            r: vtx.col[0],
                            g: vtx.col[1],
                            b: vtx.col[2],
                            a: vtx.col[3]
                        },
                        position: SDL_FPoint {
                            x: vtx.pos[0],
                            y: vtx.pos[1]
                        },
                        tex_coord: SDL_FPoint {
                            x: vtx.uv[0],
                            y: vtx.uv[1]
                        }
                    });
                }

                for idx_id in 0..list.idx_buffer().len() {
                    let idx = list.idx_buffer().get(idx_id).unwrap();
                    indices.add(idx_id).write((*idx) as c_int);
                }

                // flicker problems, if you manage to fix it lmk
                canvas.set_clip_rect(None);

                for cmd in list.commands() {
                    match cmd {
                        DrawCmd::Elements { count, cmd_params, .. } => {
                            canvas.set_clip_rect(Rect::new(cmd_params.clip_rect[0] as i32, cmd_params.clip_rect[1] as i32, (cmd_params.clip_rect[2] - cmd_params.clip_rect[0]) as u32, (cmd_params.clip_rect[3] - cmd_params.clip_rect[1]) as u32));

                            let sdl_tex = (*data).imgui_fonts_texture;
                            let texture: *mut SDL_Texture = (*sdl_tex).raw();

                            SDL_RenderGeometry(canvas.raw(), texture, vertices.offset(cmd_params.vtx_offset as isize), count as c_int, indices.offset(cmd_params.idx_offset as isize), count as c_int);
                        }

                        _ => {}
                    }
                }
            }

            // just render it all at once, it's easier lmao.
            canvas.set_clip_rect(Rect::new((512 / 2) - (420 / 2), (512 / 2) - (356 / 2), 420, 356));

            let blend_mode = SDL_ComposeCustomBlendMode(
                SDL_BlendFactor::SDL_BLENDFACTOR_SRC_ALPHA,
                SDL_BlendFactor::SDL_BLENDFACTOR_ONE_MINUS_SRC_ALPHA,
                SDL_BlendOperation::SDL_BLENDOPERATION_ADD,
                SDL_BlendFactor::SDL_BLENDFACTOR_ONE,
                SDL_BlendFactor::SDL_BLENDFACTOR_ONE_MINUS_SRC_ALPHA,
                SDL_BlendOperation::SDL_BLENDOPERATION_ADD
            );

            SDL_SetRenderDrawBlendMode(canvas.raw(), blend_mode);

            let sdl_tex = data.imgui_fonts_texture;
            let texture: *mut SDL_Texture = (*sdl_tex).raw();

            SDL_RenderGeometry(canvas.raw(), texture, vertices, draw_data.total_vtx_count as c_int, indices, draw_data.total_idx_count as c_int);

            canvas.set_clip_rect(None);

            free(vertices as *mut c_void);
            free(indices as *mut c_void);
        }
    }

    canvas.present();

    data.last_frame = current_frame;

    // "VSync"
    sleep(Duration::new(0, 1_000_000_000u32 / refresh_rate));

    true
}

unsafe fn render_ui(ui: &mut Ui, data: &mut SharedData) -> bool {
    let window = ui.window("Properties")
        .size(Vector2::from([ 420.0, 356.0 ]), Condition::Always)
        .position(Vector2::from([ (512.0 / 2.0) - (420.0 / 2.0), (512.0 / 2.0) - (356.0 / 2.0) ]), Condition::Always)
        .title_bar(true)
        .scrollable(true)
        .draw_background(true)
        .begin();

    if window.is_some() {
        let combo = ui.begin_combo("Input Device", data.input_device_name.clone());

        if combo.is_some() {
            let c = combo.unwrap();
            for device in (*data.input_devices).iter() {
                let name = device.name().unwrap();
                ui.text(name.clone());

                if name.clone() == data.input_device_name {
                    ui.set_item_default_focus();
                }

                if ui.selectable(name.clone()) {
                    data.input_device_name = name.clone();

                    for x in data.host.input_devices().unwrap() {
                        if x.name().unwrap() == name.clone() {
                            let _ = data.input_device.insert(x);
                        }
                    }
                }
            }

            c.end();
        }

        let timings = data.speech_timings;

        if ui.button("Add Timing") {
            (*timings).insert((*timings).len(), create_default_timing(data));
        }

        ui.same_line();

        if ui.button("Close Properties") {
            data.is_props_open = false;
            save(data);
        }

        ui.same_line();

        if ui.button("Exit PNGTuber") {
            return false;
        }

        for (id, timing) in (*timings).iter_mut().enumerate() {
            let group = ui.begin_group();

            if ui.collapsing_header(format!("Timing #{}##{}_group", id + 1, id), TreeNodeFlags::DEFAULT_OPEN) {
                ui.indent_by(4.0);
                if ui.button(format!("Remove##{}_remove", id)) {
                    (*data.speech_timings).remove(id);
                }

                ui.spacing();

                ui.checkbox(format!("Should Bounce?##{}_bounce", id), &mut timing.should_bounce);

                ui.text("Threshold (dB)");
                ui.slider(format!("##{}_threshold", id), 0.0, 1.0, &mut timing.threshold);

                ui.text("Attack (ms)");
                ui.slider(format!("##{}_attack", id), 0.0, 150.0, &mut timing.attack_time);

                ui.text("Release (ms)");
                ui.slider(format!("##{}_release", id), 0.0, 150.0, &mut timing.release_time);

                if timing.should_bounce {
                    ui.text("Total Bounce Frames");
                    ui.slider(format!("##{}_velocity_frames", id), 0, 600, &mut timing.total_velocity_frames);

                    ui.text("Max Bounce Velocity");
                    ui.slider(format!("##{}_max_velocity", id), 0.0, 64.0, &mut timing.max_velocity);
                }

                ui.text("Texture Path");
                ui.input_text(format!("##{}_tex_path", id), &mut timing.texture_path)
                    .build();

                ui.same_line();

                if ui.button(format!("Open Path##{}_open_path", id)) {
                    let file = FileDialog::new()
                        .add_filter("Image files", &["png", "webp"])
                        .set_title("Select Image File")
                        .pick_file();

                    if file.is_some() {
                        let file_path = file.as_ref().unwrap().to_str().unwrap();

                        timing.texture_path = String::from(file_path);

                        drop(timing.texture_surface.context());
                        unsafe {
                            // this works better than the Rust destroy because Rust is too safe.
                            SDL_DestroyTexture(timing.texture.raw());
                        }

                        let png_surface = Surface::from_file(file_path).unwrap_or(create_missing_tex());
                        // i thought this was already in unsafe but okay
                        let png_texture = unsafe { (*data.pngtuber_canvas).create_texture_from_surface(&png_surface).unwrap() };

                        timing.texture_surface = png_surface;
                        timing.texture = png_texture;
                    }
                }

                ui.spacing();
                ui.spacing();
            }

            group.end();
        }

        window.unwrap().end();
    }

    true
}