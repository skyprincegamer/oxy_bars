use audio_recorder_rs::Recorder;
use macroquad::color::{GREEN, WHITE};
use macroquad::shapes::draw_line;
use macroquad::window::next_frame;
use macroquad::window::{clear_background, screen_height, screen_width};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const ALPHA: f32 = 0.1;
const VOLUME_SCALE: f32 = 16.0;
#[macroquad::main("Texture")]
async fn main() {
    let mut recorder = Recorder::new();
    let receiver = recorder.start(true).expect("Failed to start recording");
    let samples = Arc::new(Mutex::new(VecDeque::new()));
    let samples_len = Arc::new(Mutex::new(0usize));
    let samples_len_clone = samples_len.clone();
    let samples_clone = samples.clone();
    thread::spawn(move || {
        while let Ok(d) = receiver.recv() {
            for sample in d {
                samples_clone.lock().unwrap().push_back(sample);
                *samples_len_clone.lock().unwrap() += 1;
            }
        }
    });
    let mut drawn_prev = vec![0.; screen_width() as usize];
    loop {
        if drawn_prev.len()  != screen_width() as usize {
            drawn_prev = vec![0.; screen_width() as usize];
        }
        clear_background(WHITE);
        let mid = screen_height()/2.0;
        if *samples_len.lock().unwrap() > (screen_width()*2.) as usize {
            let mut drawn: Vec<f32> = vec![0.; screen_width() as usize];
            {
                let mut locked = samples.lock().unwrap();
                for x in drawn.iter_mut() {
                    *x = ((*locked).pop_back().unwrap() * VOLUME_SCALE).min(1.0).max(-1.0);
                }
                *samples_len.lock().unwrap() = locked.len();
            }
            for (index, x) in drawn.iter_mut().enumerate() {
                *x = (drawn_prev[index] * (1.- ALPHA) + *x  * ALPHA);
            }
            draw_lines(&drawn , mid);
            for (index, x) in drawn.iter().enumerate() {
                drawn_prev[index] = *x;
            }
        }
        else{
            draw_lines(&drawn_prev, mid);
        }
        next_frame().await;
    }
    recorder.stop();
}

fn draw_lines(drawn_prev: &Vec<f32>, mid: f32) {
    for i in 0..drawn_prev.len() - 1 {
        let i_f = i as f32;
        draw_line(i_f, mid - drawn_prev[i] * mid, i_f + 1., mid - drawn_prev[i + 1] * mid, 2., GREEN);
    }
}