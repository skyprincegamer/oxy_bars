use audio_recorder_rs::Recorder;
use macroquad::color::WHITE;
use macroquad::input::is_quit_requested;
use macroquad::shapes::draw_rectangle;
use macroquad::window::next_frame;
use macroquad::window::{clear_background, screen_height, screen_width};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const SAMPLE_TIME :f64 = 1. / 44100.;
const FFT_SIZE : usize = 2048;

const GLOBAL_SCALE : f32 = 1.;
const ALPHA :f32 = 0.5;
#[macroquad::main("Texture")]
async fn main() {
    let mut recorder = Recorder::new();
    let receiver = recorder.start(true).expect("Failed to start recording");
    let samples = Arc::new(Mutex::new(VecDeque::new()));
    let samples_clone_fft = samples.clone();
    thread::spawn(move || {
        while let Ok(d) = receiver.recv() {
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
                    input[i] = locked.pop_front().unwrap();
                }
            }
            let windowed_input = hann_window(&input);
            let spectrum = samples_fft_to_spectrum(&windowed_input,
                44100, //sampling rate
               FrequencyLimit::All, None).unwrap();
            {
                let mut locked = spectrum_mutex_clone.lock().unwrap();
                for &(freq, freq_val) in spectrum.data().iter() {
                    (*locked)[(freq.val() as usize).min(44100/2 - 1)] = freq_val.val() ;
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
        let drawn = interpolate_the_bars
                                (scale_the_bars(
                                average_the_bars(locked , screen_width() as usize)) , &drawn_prev, ALPHA);
        drawn_prev = drawn.clone();
        draw_rectangles(drawn);
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


fn average_the_bars(bars: Vec<f32> , target_size : usize) -> Vec<f32>{

    let chunk_size = bars.len() / target_size;

    if target_size > bars.len() || chunk_size <= 1 || target_size == 0 {
        return bars;
    }

    let mut output : Vec<f32> = vec![0.; target_size];
    let bars : Vec<f32> = bars.iter().take(target_size*chunk_size).copied().collect();
    for i in 0..bars.len() {
        output[i/chunk_size] += bars[i];
    }
    for i in 0..output.len(){
        output[i] /= chunk_size as f32;
    }
    output
}

fn scale_the_bars(bars: Vec<f32>) -> Vec<f32>{
    let mut output = vec![0.; bars.len()];
    let scale = GLOBAL_SCALE * screen_width();
    for i in 0..output.len(){
        output[i] = bars[i] * scale;
    }
    output
}

fn interpolate_the_bars(this_bars : Vec<f32> , other : &Vec<f32> , alpha:f32) -> Vec<f32>{
    this_bars.iter().enumerate().map(|(i,&val)|{(val * alpha) + other[i] * (1.-alpha)}).collect()
}
fn draw_rectangles(bars : Vec<f32>){
    for i in 0..bars.len() {
        let i_f = i as f32;
        let height = bars[i].min(screen_height());
        let the_func = |x : f32| x / screen_height();
        let the_color = macroquad::color::Color::new(the_func(height), the_func(height), the_func(height), 1.);
        draw_rectangle(i_f, screen_height() - height, 1. , height , the_color );
    }
}