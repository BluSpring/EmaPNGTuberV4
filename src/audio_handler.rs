use std::sync::Mutex;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use cpal::{Device, Stream};
use cpal::traits::{DeviceTrait, StreamTrait};
use crate::SharedData;

pub struct SharedAudioData {
    pub(crate) current_level: f32,
    pub(crate) should_exit: bool
}

fn mul_to_db(mul: f32) -> f32 {
    return if mul == 0.0 {
        -f32::INFINITY
    } else {
        20.0f32 * mul.log10()
    };
}

pub fn spawn_audio_handler(data: &mut SharedData) -> Stream {
    let audio_data = unsafe { &mut *data.audio_data };
    let fuck_off = std::mem::take(&mut data.input_device);
    let device = fuck_off.unwrap();

    let config = device.default_input_config().unwrap().config();

    let stream = device.build_input_stream(&config,
        move | d: &[f32], info: &cpal::InputCallbackInfo | {
            let mut sum = 0.0f32;
            for x in d {
                sum += *x * *x;
            }

            audio_data.current_level = sum.sqrt();
        },
        move |err| {
            eprintln!("{}", err);
        },
        None
    ).unwrap();

    stream.play().unwrap();

    stream
}