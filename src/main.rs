use macroquad::prelude::*;
use std::sync::{Arc, Mutex};
use std::{fs, thread};
use std::time::Duration;
use bar_gen::ColorRgbtuple;
mod bar_gen;


#[macroquad::main("GPU Spectrum")]
async fn main() {
    // Shared data
    let shared_bars = Arc::new(Mutex::new(Arc::new(vec![])));
    let shared_size = Arc::new(Mutex::new((screen_width(), screen_height())));
    let shared_colors = Arc::new(Mutex::new(Arc::new(vec![])));
    // Start audio thread
    let bars_clone = shared_bars.clone();
    let colors_clone = shared_colors.clone();

    thread::spawn(move|| {
        bar_gen::start_audio_thread(bars_clone, shared_size.clone() , colors_clone);
    });
    thread::sleep(Duration::from_secs(2));
    let content = fs::read_to_string("config.toml").expect("Failed to read file");

    // Parse into toml::Value
    let value: toml::Value = toml::from_str(&content).expect("Invalid TOML");

    let final_color  = value.get("color_top").expect("Failed to get final_color").to_string();
    let sigma = value.get("sigma").expect("Failed to get alpha").to_string().parse::<f32>().unwrap();
    let final_color : ColorRgbtuple =  {
        let nums: Vec<u8> = final_color
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .map(|x| x.trim().parse().expect("Invalid number"))
            .collect();
        (nums[0], nums[1], nums[2])
    };



    let mut old_bars: Option<Vec<f32>> = None;
    loop {
        clear_background(BLACK);
        let bars = {
            let guard = shared_bars.lock().unwrap();
            guard.clone()
        };
        let colors = {
            let guard = shared_colors.lock().unwrap();
            guard.clone()
        };
        let bars = if let Some(prev) = &old_bars {
            // Make sure lengths match
            if prev.len() == bars.len() {
                prev.iter()
                    .zip(bars.iter())
                    .map(|(old, new)| old * (1.0 - sigma) + new * sigma)
                    .collect::<Vec<f32>>()
            } else {
                (*bars).clone()
            }
        } else {
            (*bars).clone()
        };
        old_bars = Some(bars.clone());


        if !bars.is_empty() {
            let screen_w = screen_width();
            let screen_h = screen_height();
            let bar_count = bars.len();
            let bar_width = screen_w / bar_count as f32 /2.0;

            for (i, &bar_height) in bars.iter().enumerate() {
                let (r , g , b) = bar_gen::get_rgb_tuple(bar_height as usize , colors[i] , final_color , screen_h as usize);
                let (r , g , b) = (r as f32/255.0, g as f32/255.0 , b as f32 /255.0);
                let bar_height = bar_height.min(screen_h - (0.001*screen_h).max(5.0));
                draw_rectangle(
                    i as f32 * bar_width,
                    screen_h - bar_height,
                    bar_width,
                    bar_height ,
                    Color { r , g , b , a: 1.0},
                );
                draw_rectangle(
                    screen_w - i as f32 * bar_width,
                    screen_h - bar_height,
                    bar_width,
                    bar_height ,
                    Color { r , g , b , a: 1.0},
                );
            }
        }

        next_frame().await;
    }
}
