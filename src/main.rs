mod utils;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use macroquad::color::BLACK;
use macroquad::input::is_quit_requested;
use macroquad::window::{clear_background, Conf};
use macroquad::window::next_frame;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use windowfunctions::window;

#[derive(Deserialize, Serialize, Debug)]
struct Config {
    fft: usize,
    alpha: f32,
    color_start: Vec<f32>,
    color_end: Vec<f32>,
    color_final: Vec<f32>,
    scale : u32,
    falling_factor:u32
}

impl Default for Config {
    fn default() -> Self {
        Config {
            fft: 2048,
            alpha: 0.5,
            color_start: vec![1.0, 0.5, 0.0],
            color_end: vec![0.5, 1.0, 0.0],
            color_final: vec![0.5, 0.5, 1.0],
            scale :2,
            falling_factor: 10
        }
    }
}
fn window_conf() -> Conf {
    Conf {
        window_title: "Oxy Bars".to_owned(),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut config_str = String::new();
    let config_file = File::open("config.toml");
    let mut erred = false;
    match config_file {
        Ok(mut file) => {
            let res = file.read_to_string(&mut config_str);
            match res {
                Ok(_) => {}
                Err(_) => {
                    config_str = "".to_string();
                    erred = true
                }
            }
        }
        Err(_) => {
            config_str = "".to_string();
            erred = true
        }
    }
    dbg!(erred);
    let config: Config;
    let config_res = toml::from_str(config_str.as_str());
    match config_res {
        Ok(x) => config = x,
        Err(_) => {
            erred = true;
            config = Config::default()
        }
    }
    dbg!(&config);
    if erred {
        let mut f = File::create("config.toml").unwrap();
        f.write_all(toml::to_string(&config).unwrap().as_bytes())
            .unwrap();
        dbg!("file written!");
    }
    let device = cpal::default_host()
        .default_input_device()
        .expect("NO MIC BRUH");
    let device_config = device.default_input_config().expect("NO MIC CONFIG BRUH");
    let samples = Arc::new(Mutex::new(VecDeque::new()));
    let samples_clone_fft = samples.clone();
    let noise_profile = Arc::new(Mutex::new(Vec::<f32>::with_capacity(config.fft)));
    let noise_profile_clone = noise_profile.clone();
    let noise_recording_done = Arc::new(AtomicBool::new(false));
    let noise_recording_done_clone = noise_recording_done.clone();
    let noise_spectrum_done = Arc::new(AtomicBool::new(false));
    let noise_spectrum_done_clone = noise_spectrum_done.clone();

    //recorder stream
    let stream = device
        .build_input_stream(
            &device_config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !noise_recording_done.load(SeqCst) {
                    let mut noise = noise_profile.lock().unwrap();
                    for &sample in data {
                        if noise.len() < config.fft {
                            noise.push(sample);
                        }
                    }
                    if noise.len() >= config.fft {
                        noise_recording_done.store(true, SeqCst);
                    }
                } else {
                    let mut locked = samples.lock().unwrap();
                    for &sample in data {
                        locked.push_back(sample);
                    }
                }
            },
            |err| eprintln!("stream error: {err}"),
            None,
        )
        .expect("Failed to build input stream");

    stream.play().expect("Failed to start stream");
    let spectrum_mutex: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(vec![0.; config.fft]));
    let spectrum_mutex_clone = spectrum_mutex.clone();

    //fft thread
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(config.fft);
    thread::spawn(move || {
        let mut noise_noise = vec![];
        loop {
            thread::sleep(Duration::from_micros(22)); // 1/44100 s = 22 µs

            let mut buffer = vec![Complex { re: 0.0, im: 0.0 }; config.fft];

            if !noise_spectrum_done.load(SeqCst) && noise_recording_done_clone.load(SeqCst) {
                let mut locked = noise_profile_clone.lock().unwrap();
                buffer = locked.iter().map(|&x| Complex { re: x, im: 0. }).collect();
                fft.process(&mut buffer);
                noise_noise = buffer.iter().map(|&x| x.norm()).collect();
                noise_spectrum_done.store(true, SeqCst);
            } else if !noise_recording_done_clone.load(SeqCst) {
                continue;
            } else {
                let mut locked = samples_clone_fft.lock().unwrap();
                if locked.len() < config.fft + 2 {
                    continue;
                }
                let mut raw_buffer = vec![0.0;config.fft];
                for i in 0..config.fft {
                    raw_buffer[i] = locked.pop_front().unwrap();
                }
                //windowing for reducing spectral leakage

                let length = config.fft;
                let window_type = windowfunctions::WindowFunction::BlackmanNuttall;
                let symmetry = windowfunctions::Symmetry::Symmetric;

                let window_iter = window::<f32>(length, window_type, symmetry);

                let window_vec: Vec<f32> = window_iter.into_iter().collect();
                for i in 0..config.fft{
                    raw_buffer[i] *= window_vec[i];
                }
                buffer = raw_buffer.into_iter().map(|x| Complex { re: x, im: 0. }).collect();
                // windowing done

                fft.process(&mut buffer);
                let mut locked = spectrum_mutex_clone.lock().unwrap();
                for i in 0..config.fft{
                    locked[i] = (buffer[i].norm() - noise_noise[i]).max(0.0);
                }
            }
        }
    });
    let disp_w = config.fft/2 - (config.fft/100) * 2;
    let mut drawn_prev = vec![0.; disp_w];
    loop {
        clear_background(BLACK);
        if !noise_spectrum_done_clone.load(SeqCst) {
            continue;
        }
        let locked = spectrum_mutex.lock().unwrap().clone();
        drawn_prev = utils::draw_rectangles(&locked, &drawn_prev, &config);
        match macroquad::input::get_char_pressed() {
            None => {}
            Some(key) => {
                if key == char::from_u32(27).unwrap() {
                    break;
                }
            }
        }
        if is_quit_requested() {
            break;
        }
        next_frame().await;
    }
}

