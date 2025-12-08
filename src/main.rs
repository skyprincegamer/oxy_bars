use audio_recorder_rs::Recorder;
use macroquad::color::{BLACK, GREEN, RED, WHITE};
use macroquad::shapes::draw_line;
use macroquad::window::next_frame;
use macroquad::window::{clear_background, screen_height, screen_width};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};
use std::thread;
use macroquad::camera::{set_camera, set_default_camera};
use macroquad::math::Vec2;
use macroquad::prelude::{draw_texture, Camera2D};
use macroquad::texture::{render_target, RenderTarget, Texture2D};

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
            let mut deq = samples_clone.lock().unwrap();
            for sample in d {
                deq.push_back(sample);
                *samples_len_clone.lock().unwrap() += 1;
            }
        }
    });

    loop {
        clear_background(WHITE);
        let mid = screen_height()/2.0;
        if *samples_len.lock().unwrap() > screen_width() as usize {
            let mut drawn: Vec<f32> = vec![0.; screen_width() as usize];
            {
                let mut locked = samples.lock().unwrap();
                for x in drawn.iter_mut() {
                    *x = (*locked).pop_back().unwrap();
                }
                *samples_len.lock().unwrap() = 0;
            }

            for i in 0..drawn.len()-1 {
                let iF = i as f32;
                draw_line(iF, mid - drawn[i]*mid, iF+1., mid - drawn[i+1]*mid, 2., GREEN);
            }
        }
        next_frame().await;
    }
    recorder.stop();
}