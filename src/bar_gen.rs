use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{num_complex::Complex, FftPlanner};
use std::io::{stdout, Write};
use owo_colors::OwoColorize;
use windowfunctions;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use toml;
use terminal_size::{Width, terminal_size, Height};
use savgol_rs;
use savgol_rs::{savgol_filter, SavGolInput};

type ColorRgbtuple = (u8, u8, u8);

fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
}

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
        );

        fs::write(path, toml_str).expect("Failed to write default config with comments");
        println!("Created default config at {}", path);
        default_config
    }
}


fn get_terminal_width() -> usize {
    if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        80
    }
}

fn get_terminal_height() -> usize {
    if let Some((_, Height(h))) = terminal_size() {
        h as usize
    } else {
        24
    }
}

fn spatial_smooth_bins(bar_lengths: &Vec<f32>) -> Vec<f32> {
    let sav = SavGolInput { data: bar_lengths, window_length: 5, poly_order: 3, derivative: 2 };
    savgol_filter(&sav).unwrap().iter().map(|&x| x as f32).collect()
}
fn print_horizontal_spectrum(magnitudes: &[f32], config: &Config) {
    let mut num_bars = (get_terminal_height() - 5).max(10);
    num_bars = num_bars * 2;

    clear_screen();

    let first_nonzero = magnitudes.iter().position(|&x| x > 0.0).unwrap_or(0);
    let last_nonzero = magnitudes.iter().rposition(|&x| x > 0.0).unwrap_or(magnitudes.len() - 1);

    if last_nonzero < first_nonzero {
        return;
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

    let term_width = get_terminal_width();
    let scale = term_width as f32 / config.max_magnitude;
    bars = spatial_smooth_bins(&bars);
    for (i, &magnitude) in bars.iter().take(bars.len() / 2).enumerate().rev() {
        let bar_length = (magnitude * scale) as usize;
        let (r, g, b) = get_rgb_tuple(i, config.color_top , config.color_bottom , num_bars);
        let clamped_length = bar_length.min(term_width);
        println!("{}", "█".repeat(clamped_length.max(1)).truecolor(r, g, b));
    }

    stdout().flush().unwrap();
}

fn get_rgb_tuple(i: usize, color_start: ColorRgbtuple, color_end: ColorRgbtuple, max: usize) -> ColorRgbtuple {
    let factor = (i as f32) / (max as f32);
    let (red_a, green_a, blue_a) = color_start;
    let (red_b, green_b, blue_b) = color_end;
    let f = |start, end| (start as f32 + factor * (end as f32 - start as f32)) as u8;
    (f(red_a, red_b), f(green_a, green_b), f(blue_a, blue_b))
}

fn cpal_func() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_config("config.toml");

    //SPECTRAL LEAKAGE FIX
    let window_type = windowfunctions::WindowFunction::Bartlett;
    let symmetry = windowfunctions::Symmetry::Symmetric;
    let win_iter = windowfunctions::window::<f64>(config.fft_size, window_type, symmetry);
    let hann_win: Vec<f64> = win_iter.take(config.fft_size).collect();

    let host = cpal::default_host();
    let device = host.default_input_device().expect("No input device available");
    let config_in = device.default_input_config()?;
    let sample_rate: f32 = config_in.sample_rate().0 as f32;
    let delta_f: f32 = sample_rate / config.fft_size as f32;

    let skip_bin_index: usize = (config.f_min / delta_f) as usize;
    let take_bin_index: usize = ((config.f_max / delta_f) as usize) - skip_bin_index;

    let color_top = config.color_top;
    let color_bottom = config.color_bottom;

    println!("Input device: {}", device.name()?);
    println!("Default input config: {:?}", config_in);
    println!("Profiling noise for {} seconds...", config.noise_profile_time);
    println!("Press Ctrl+C to quit.\n");

    let mut buffer: Vec<f32> = Vec::with_capacity(config.fft_size);
    let mut smoothed_mags = vec![0.0f32; config.fft_size / 2];
    let mut noise_profile = vec![0.0f32; config.fft_size / 2];

    let mut profiling = true;
    let mut profiled_frames = 0usize;
    let required_frames = (sample_rate as usize * config.noise_profile_time as usize) / config.fft_size;




    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(config.fft_size);

    let stream = device.build_input_stream(
        &config_in.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            buffer.extend_from_slice(data);

            while buffer.len() >= config.fft_size {
                let mut input: Vec<Complex<f32>> = buffer[..config.fft_size]
                    .iter()
                    .enumerate()
                    .map(|(i, &x)| x * hann_win[i] as f32)
                    .map(|x| Complex { re: x, im: 0.0 })
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
                        eprintln!("Noise profile complete. Starting spectrum visualization...");
                    }
                } else {
                    for (i, &mag) in mags.iter().enumerate() {
                        let clean_mag = (mag - noise_profile[i]).max(0.0);
                        smoothed_mags[i] = smoothed_mags[i] * (1.0 - config.alpha) + clean_mag * config.alpha;
                    }
                    print_horizontal_spectrum(&smoothed_mags, &config);
                }
            }
        },
        move |err| eprintln!("Stream error: {}", err),
        None,
    )?;

    stream.play()?;
    std::thread::park();
    Ok(())
}
