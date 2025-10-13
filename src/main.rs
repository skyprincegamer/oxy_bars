mod bar_gen;

use minifb::{Key, Window, WindowOptions};
use std::time::{Duration, Instant};

fn main() {
    let mut buffer: Vec<u32> = vec![0x00FF00; 800 * 600];
    let mut window = Window::new("Test", 800, 600, WindowOptions::default()).unwrap();

    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);
    println!("Frame time: {:?}", frame_duration);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let start = Instant::now();
        for pixel in buffer.iter_mut() {
            *pixel = (*pixel + 0x000011) & 0xFFFFFF; // Keep it in RGB range
        }
        window.update_with_buffer(&buffer, 800, 600).unwrap();

        let elapsed = start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }
}