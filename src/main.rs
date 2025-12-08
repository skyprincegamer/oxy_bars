use audio_recorder_rs::Recorder;
use macroquad::color::{BLACK, GREEN};
use macroquad::shapes::draw_rectangle;
use macroquad::window::{clear_background};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use macroquad::miniquad::window::set_window_size;
use macroquad::window::next_frame;

#[macroquad::main("Texture")]
async fn main() {
    let mut recorder = Recorder::new();
    let receiver = recorder.start(true).expect("Failed to start recording");
    let screen_width = 1024;
    let screen_height = 1024;
    let samples = Arc::new(Mutex::new(VecDeque::new()));
    let samples_clone = samples.clone();
    thread::spawn(move || {
        while let Ok(d) = receiver.recv() {
            for sample in d {
                samples_clone.lock().unwrap().push_back(sample);
            }
        }
    });
    clear_background(BLACK);
    set_window_size(screen_width, screen_height);
    loop{
        if samples.lock().unwrap().len() > screen_width as usize {
            for (index, sample) in samples.lock().unwrap().drain(..screen_width as usize).enumerate() {
                draw_rectangle(index as f32, (screen_width as f32 / 2.0) - (sample * (screen_width as f32 / 2.0)), 10., 10., GREEN);
            }
            println!("check macro if")
        }
        else {
            continue;
        }
        next_frame().await;
    }
    recorder.stop();
}