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
        window_title: String::from("Oxy Bars"),
        ..Default::default()
    }
}

fn get_config_path() -> String {
    let is_appimage = std::env::var("APPIMAGE").is_ok();

    if is_appimage {
        let home = std::env::var("HOME").expect("HOME environment variable not set");
        let config_path = home + "/.config/oxy_bars/";
        let _ = std::fs::create_dir_all(&config_path);
        config_path + "config.toml"
    } else {
        String::from("config.toml")
    }
}
fn find_config() -> Option<Config> {
    let config_path = get_config_path();
    let config_file = File::open(config_path);
    match config_file {
        Ok(mut file) => {
            let mut contents = String::new();
            let res = file.read_to_string(&mut contents);
            if res.is_err() {
                None
            }
            else{
                let res = toml::from_str::<Config>(&contents);
                res.ok()
            }
        }
        Err(_) => None
    }
}
fn write_config() -> std::io::Result<()> {
    let path = get_config_path();
    let mut config_file = File::create(path)?;
    let default = Config::default();
    config_file.write_all(toml::to_string(&default).unwrap().as_bytes())
}
#[macroquad::main(window_conf)]
async fn main() {
    let config = find_config().unwrap_or_else(|| {
        if write_config().is_err() {
            panic!("Could not generate config file")
        }
        Config::default()
    });
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

            let mut buffer : Vec<Complex<f32>>;

            if !noise_spectrum_done.load(SeqCst) && noise_recording_done_clone.load(SeqCst) {
                let locked = noise_profile_clone.lock().unwrap();
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

