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

const NOISE_PROFILE_TIME: f32 = 3.0; // seconds to capture noise
const NOISE_ALPHA: f32 = 0.05;       // learning rate for noise profile

fn print_horizontal_spectrum(magnitudes: &[f32], mut num_bars: usize, term_width: usize) {
    clear_screen();
    num_bars = num_bars * 2;
    let bins_per_bar = magnitudes.len() / num_bars;
    let mut bars: Vec<f32> = Vec::with_capacity(num_bars);

    for bar_index in 0..num_bars {
        let start_bin = bar_index * bins_per_bar;
        let end_bin = start_bin + bins_per_bar;
        let chunk = &magnitudes[start_bin..end_bin];
        let avg_magnitude = if !chunk.is_empty() {
            chunk.iter().sum::<f32>() / chunk.len() as f32
        } else {
            0.0
        };
        bars.push(avg_magnitude);
    }

    let scale = term_width as f32 / MAX_MAGNITUDE;

    for (i, &magnitude) in bars.iter().take(bars.len() / 2).enumerate() {
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
                    .take(FFT_SIZE / 2)
                    .map(|c| c.norm())
                    .collect();

                if profiling {
                    // Build noise profile directly without smoothing
                    for (i, &mag) in mags.iter().enumerate() {
                        noise_profile[i] = mag.max(noise_profile[i]);
                    }
                    profiled_frames += 1;

                    if profiled_frames >= required_frames {
                        profiling = false;
                        eprintln!("Noise profile complete. Starting spectrum visualization...");
                    }
                } else {
                    // Apply noise cancellation (keep ALPHA smoothing for display)
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
