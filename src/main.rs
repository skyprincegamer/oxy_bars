mod utils;

use audio_recorder_rs::Recorder;
use macroquad::color::WHITE;
use macroquad::input::is_quit_requested;
use macroquad::window::next_frame;
use macroquad::window::{clear_background, screen_width};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{samples_fft_to_spectrum, Frequency, FrequencyLimit};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const SAMPLE_TIME :f64 = 1_000_000. / 44100.;
const FFT_SIZE : usize = 2048;

const GLOBAL_SCALE : f32 = 5.;
const ALPHA :f32 = 0.5;
const COLOR_START : (f32, f32, f32) = (1., 0.5, 0.);
const COLOR_END : (f32, f32, f32) = (0.5, 1.0, 0.);
const COLOR_FINAL : (f32, f32, f32) = (0.5, 0.5, 1.0);

#[macroquad::main("Oxy Bars")]
async fn main() {
    let mut recorder = Recorder::new();
    let receiver = recorder.start(true).expect("Failed to start recording");
    let samples = Arc::new(Mutex::new(VecDeque::new()));
    let samples_clone_fft = samples.clone();
    let noise_profile = Arc::new(Mutex::new(Vec::<f32>::with_capacity(FFT_SIZE)));
    let noise_profile_clone = noise_profile.clone();
    let mut noise_profiling_done = false;
    thread::spawn(move || {
        while let Ok(d) = receiver.recv() {
            while !noise_profiling_done && noise_profile.lock().unwrap().len() < FFT_SIZE  {
                let mut locked = noise_profile.lock().unwrap();
                for sample in d.clone() {
                    if locked.len() < FFT_SIZE {
                        locked.push(sample);
                    }
                }
            }
            noise_profiling_done = true;
            for sample in d {
                samples.lock().unwrap().push_back(sample);
            }
        }
    });
    let spectrum_mutex :Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(vec![0. ; 44100/2]));
    let spectrum_mutex_clone = spectrum_mutex.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_micros(SAMPLE_TIME as u64));
        let mut input = vec![0.; FFT_SIZE];
        if samples_clone_fft.lock().unwrap().len() > (FFT_SIZE+2) {
            {
                let mut locked = samples_clone_fft.lock().unwrap();
                for i in 0..FFT_SIZE {
                    input[i] = locked.pop_front().unwrap() ;
                }
            }
            let mut spectrum_noise = None;
            if noise_profile_clone.lock().unwrap().len() >= (FFT_SIZE) {
                let locked_noise = noise_profile_clone.lock().unwrap();
                let windowed_noise = hann_window(&locked_noise);
                spectrum_noise = Some(samples_fft_to_spectrum(&windowed_noise,
                                                             44100, //sampling rate
                                                             FrequencyLimit::All, None).unwrap());
            }
            else{
                continue;
            }

            let windowed_input = hann_window(&input);
            let spectrum = samples_fft_to_spectrum(&windowed_input,
                44100, //sampling rate
               FrequencyLimit::All, None).unwrap();

            {
                let mut locked = spectrum_mutex_clone.lock().unwrap();
                let indexer = |x:Frequency| (x.val() as usize).min(44100/2 - 1);
                let noise_iter = spectrum_noise.as_ref().unwrap().data().iter();
                for (&(freq, freq_val) , &(noise_freq , noise_val)) in spectrum.data().iter().zip(noise_iter) {
                    if indexer(noise_freq) == indexer(freq){
                        (*locked)[indexer(freq)] = (freq_val.val() - noise_val.val()).max(0.0);
                    }
                    else{
                        (*locked)[indexer(freq)] = freq_val.val();
                    }
                    
                }
            }
        }
    });
    let mut drawn_prev = vec![0.; screen_width() as usize];
    loop {

        clear_background(WHITE);
        if drawn_prev.len() != screen_width() as usize {
            drawn_prev = vec![0.; screen_width() as usize];
        }
        let locked = spectrum_mutex.lock().unwrap().clone();
        let drawn = utils::interpolate_the_bars
                                (utils::scale_the_bars(
                                utils::average_the_bars(locked , screen_width() as usize)) , &drawn_prev, ALPHA);
        drawn_prev = drawn.clone();
        utils::draw_rectangles(drawn);
        match macroquad::input::get_char_pressed(){
            None => {}
            Some(key) => {
                if key == char::from_u32(27).unwrap(){
                    break;
                }
            }
        }
        if is_quit_requested(){
            break;
        }
        next_frame().await;
    }
    recorder.stop();
}