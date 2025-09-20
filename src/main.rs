use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{num_complex::Complex, FftPlanner};
use std::io::{stdout, Write};
use owo_colors::OwoColorize;
use apodize::*;

type ColorRgbtuple = (u8, u8, u8);

fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
}

const SAMPLE_RATE: f32 = 44_100.0;
const FFT_SIZE: usize = 2048;
const ALPHA: f32 = 0.2;
const MAX_MAGNITUDE: f32 = 50.0;

const NUM_BARS: usize = 60;
const TERM_WIDTH: usize = 200;

const NOISE_PROFILE_TIME: f32 = 3.0;

const DELTA_F: f32 = SAMPLE_RATE / FFT_SIZE as f32;
const F_MIN: f32 = 100.0;
const F_MAX: f32 = 2000.0;

const SKIP_BIN_INDEX: usize = (F_MIN / DELTA_F) as usize;
const TAKE_BIN_INDEX: usize = ((F_MAX / DELTA_F) as usize) - SKIP_BIN_INDEX;

fn print_horizontal_spectrum(magnitudes: &[f32], mut num_bars: usize, term_width: usize) {
    num_bars = num_bars *2;
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
        let start_bin = start_bin.min(len - 1); // clamp to prevent overflow
        let avg_magnitude = active_slice[start_bin];
        bars.push(avg_magnitude);
    }

    let scale = term_width as f32 / MAX_MAGNITUDE;

    for (i, &magnitude) in bars.iter().take(bars.len()/2).enumerate() {
        let bar_length = (magnitude * scale) as usize;
        let (r, g, b) = get_rgb_tuple(i, (48, 33, 147), (147, 33, 143), num_bars);
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("No input device available");
    let config = device.default_input_config()?;

    println!("Input device: {}", device.name()?);
    println!("Default input config: {:?}", config);
    println!("Profiling noise for {} seconds...", NOISE_PROFILE_TIME);
    println!("Press Ctrl+C to quit.\n");

    let mut buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE);
    let mut smoothed_mags = vec![0.0f32; FFT_SIZE / 2];
    let mut noise_profile = vec![0.0f32; FFT_SIZE / 2];

    let mut profiling = true;
    let mut profiled_frames = 0usize;
    let required_frames = (SAMPLE_RATE as usize * NOISE_PROFILE_TIME as usize) / FFT_SIZE;

    let hann_win: Vec<f64> = hanning_iter(FFT_SIZE).collect();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            buffer.extend_from_slice(data);

            while buffer.len() >= FFT_SIZE {
                let mut input: Vec<Complex<f32>> = buffer[..FFT_SIZE]
                    .iter()
                    .enumerate()
                    .map(|(i, &x)| x * hann_win[i] as f32)
                    .map(|x| Complex { re: x, im: 0.0 })
                    .collect();
                buffer.drain(..FFT_SIZE);

                fft.process(&mut input);
                let mags: Vec<f32> = input
                    .iter()
                    .skip(SKIP_BIN_INDEX)
                    .take(TAKE_BIN_INDEX)
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
                        smoothed_mags[i] = smoothed_mags[i] * (1.0 - ALPHA) + clean_mag * ALPHA;
                    }

                    print_horizontal_spectrum(&smoothed_mags, NUM_BARS, TERM_WIDTH);
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
