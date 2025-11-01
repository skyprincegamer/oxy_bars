use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use savgol_rs;
use savgol_rs::{savgol_filter, SavGolInput};
use serde::{Deserialize, Serialize};
use std::{fs, ops, path::Path};
use toml;
use windowfunctions;


pub type ColorRgbtuple = (u8, u8, u8);
type ScreenSize = (f32 , f32);


#[derive(Debug, Deserialize, Serialize)]
struct Config {
    fft_size: usize,
    alpha: f32,
    max_magnitude: f32,
    noise_profile_time: f32,
    f_min: f32,
    f_max: f32,
    color_top : ColorRgbtuple,
    color_bottom : ColorRgbtuple,
    final_color: ColorRgbtuple,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            fft_size: 512,
            alpha: 0.05,
            max_magnitude: 10.0,
            noise_profile_time: 3.0,
            f_min: 100.0,
            f_max: 10_000.0,
            color_top : (48, 33, 147),
            color_bottom : (147, 33, 143),
            final_color : (255, 255, 255)
        }
    }
}


fn load_or_create_config(path: &str) -> Config {
    if Path::new(path).exists() {
        let content = fs::read_to_string(path).expect("Failed to read config file");
        toml::from_str(&content).expect("Invalid TOML in config file")
    } else {
        let default_config = Config::default();

        // Manually create TOML with comments
        let toml_str = format!(
            r#"# Spectrum Analyzer Configuration
# fft_size: Number of samples per FFT (higher = better frequency resolution, slower refresh)
fft_size = {fft_size}

# alpha: Temporal smoothing factor for magnitudes (0.0 = no smoothing, 1.0 = max smoothing)
alpha = {alpha}

# max_magnitude: Controls scaling of bar length in the terminal
max_magnitude = {max_magnitude}

# noise_profile_time: Seconds to measure ambient noise before visualization starts
noise_profile_time = {noise_profile_time}

# f_min and f_max: Frequency range to display (Hz)
f_min = {f_min}
f_max = {f_max}

# color_top and color_bottom: RGB tuples for gradient
color_top = [{r1}, {g1}, {b1}]
color_bottom = [{r2}, {g2}, {b2}]
final_color = [{r3}, {g3}, {b3}]
"#,
            fft_size = default_config.fft_size,
            alpha = default_config.alpha,
            max_magnitude = default_config.max_magnitude,
            noise_profile_time = default_config.noise_profile_time,
            f_min = default_config.f_min,
            f_max = default_config.f_max,
            r1 = default_config.color_top.0,
            g1 = default_config.color_top.1,
            b1 = default_config.color_top.2,
            r2 = default_config.color_bottom.0,
            g2 = default_config.color_bottom.1,
            b2 = default_config.color_bottom.2,
            r3 = default_config.final_color.0,
            g3 = default_config.final_color.1,
            b3 = default_config.final_color.2,
        );

        fs::write(path, toml_str).expect("Failed to write default config with comments");
        println!("Created default config at {}", path);
        default_config
    }
}




fn spatial_smooth_bins(bar_lengths: &mut Vec<f32>) {
    let sav = SavGolInput { data: bar_lengths, window_length: 7, poly_order: 2, derivative: 0 };
    *bar_lengths = savgol_filter(&sav).unwrap().iter().map(|&x| x as f32).collect()
}
fn print_horizontal_spectrum(s: ScreenSize , magnitudes: &[f32], config: &Config) -> Vec<f32>{
    let (width, height) = s;
    let mut num_bars = width as usize;
    num_bars = num_bars * 2;

    let first_nonzero = magnitudes.iter().position(|&x| x > 0.0).unwrap_or(0);
    let last_nonzero = magnitudes.iter().rposition(|&x| x > 0.0).unwrap_or(magnitudes.len() - 1);

    if last_nonzero < first_nonzero {
        return vec![];
    }

    let active_slice = &magnitudes[first_nonzero..=last_nonzero];
    let len = active_slice.len();
    let mut bars: Vec<f32> = Vec::with_capacity(num_bars);

    let scale_factor = len as f32 / num_bars as f32;
    for bar_index in 0..num_bars {
        let start_bin = (bar_index as f32 * scale_factor).round() as usize;
        let start_bin = start_bin.min(len - 1);
        let avg_magnitude = active_slice[start_bin];
        bars.push(avg_magnitude);
    }

    let scale = height /config.max_magnitude;
    for b in bars.iter_mut(){
        *b = *b * scale;
    }
    spatial_smooth_bins(&mut bars);
    bars.into_iter().take(num_bars/2).rev().collect()
}

 pub(crate) fn get_rgb_tuple(i: usize, color_start: ColorRgbtuple, color_end: ColorRgbtuple, max: usize) -> ColorRgbtuple {
    let factor = (i as f32) / (max as f32);
    let (red_a, green_a, blue_a) = color_start;
    let (red_b, green_b, blue_b) = color_end;
    let f = |start, end| (start as f32 + factor * (end as f32 - start as f32)) as u8;
    (f(red_a, red_b), f(green_a, green_b), f(blue_a, blue_b))
}

fn get_shared_colors(len : usize , config: &Config) -> Vec<ColorRgbtuple> {
    (0..len).map(|index| get_rgb_tuple(index , config.color_top , config.color_bottom , len)).collect()
}

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn start_audio_thread(shared_bars: Arc<Mutex<Arc<Vec<f32>>>>, s : Arc<Mutex<ScreenSize>>, c : Arc<Mutex<Arc<Vec<ColorRgbtuple>>>> )  {
    thread::spawn(move || {
        // everything in your cpal_func goes here, except the infinite park

        let config = load_or_create_config("config.toml");
        let window_type = windowfunctions::WindowFunction::BlackmanNuttall;
        let symmetry = windowfunctions::Symmetry::Symmetric;
        let win_iter = windowfunctions::window::<f64>(config.fft_size, window_type, symmetry);
        let hann_win: Vec<f64> = win_iter.take(config.fft_size).collect();

        let host = cpal::default_host();
        let device = host.default_input_device().expect("No input device");
        let config_in = device.default_input_config().unwrap();
        let sample_rate = config_in.sample_rate().0 as f32;
        let delta_f = sample_rate / config.fft_size as f32;
        let skip_bin_index = (config.f_min / delta_f) as usize;
        let take_bin_index = ((config.f_max / delta_f) as usize) - skip_bin_index;

        let mut buffer: Vec<f32> = Vec::with_capacity(config.fft_size);
        let mut noise_profile = vec![0.0f32; config.fft_size / 2];
        let mut profiling = true;
        let mut profiled_frames = 0usize;
        let required_frames = (sample_rate as usize * config.noise_profile_time as usize) / config.fft_size;
        let mut planner = rustfft::FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(config.fft_size);

        let stream = device.build_input_stream(
            &config_in.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                buffer.extend_from_slice(data);
                while buffer.len() >= config.fft_size {
                    let mut input: Vec<rustfft::num_complex::Complex<f32>> = buffer[..config.fft_size]
                        .iter()
                        .enumerate()
                        .map(|(i, &x)| x * hann_win[i] as f32)
                        .map(|x| rustfft::num_complex::Complex { re: x, im: 0.0 })
                        .collect();
                    buffer.drain(..config.fft_size);
                    fft.process(&mut input);

                    let mags: Vec<f32> = input
                        .iter()
                        .skip(skip_bin_index)
                        .take(take_bin_index)
                        .map(|c| c.norm())
                        .collect();

                    if profiling {
                        for (i, &mag) in mags.iter().enumerate() {
                            noise_profile[i] = mag.max(noise_profile[i]);
                        }
                        profiled_frames += 1;
                        if profiled_frames >= required_frames {
                            profiling = false;
                            eprintln!("Noise profiling complete.");
                        }
                    } else {

                        let mags : Vec<f32> = mags.iter().enumerate().map(|(i , elem)| (elem - noise_profile[i]).max(0.0)).collect();

                        let size = {
                            let guard = s.lock().unwrap();
                            *guard
                        };
                        let bars = print_horizontal_spectrum(size, &mags, &config);
                        // share bars with GUI
                        if let Ok(mut shared) = shared_bars.lock() {
                            *shared = Arc::new(bars);
                        }
                        let colors = get_shared_colors(size.0 as usize, &config);
                        if let Ok(mut shared) = c.lock() {
                            *shared = Arc::new(colors);
                        }
                    }
                }
            },
            move |err| eprintln!("Stream error: {}", err),
            None,
        ).unwrap();

        stream.play().unwrap();
        thread::park();
    });
}