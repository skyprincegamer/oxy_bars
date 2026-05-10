use macroquad::color::Color;
use macroquad::prelude::{draw_rectangle, screen_height, screen_width};
use crate::Config;


pub fn scale_the_bars(bars: Vec<f32> , scale : u32) -> Vec<f32>{
    let mut output = vec![0.; bars.len()];
    for i in 0..output.len(){
        output[i] = bars[i] * scale as f32;
    }
    output
}

pub fn interpolate_the_bars(this_bars : &Vec<f32> , other : &Vec<f32> , alpha:f32) -> Vec<f32>{
    this_bars.iter().enumerate().map(|(i,&val)|{(val * alpha) + other[i] * (1.-alpha)}).collect()
}
pub fn draw_rectangles(spec: &Vec<f32> ,prev : &Vec<f32>, config: &Config) -> Vec<f32>{
    if spec.len() <= config.fft/2 - 1{
        prev.clone()
    }
    else {
        let BUFFER = config.fft/100;
        let bars = interpolate_the_bars(&scale_the_bars(spec.into_iter().cloned().skip(BUFFER).take(config.fft / 2 - BUFFER*2).collect(), config.scale), prev, config.alpha);

        let n = bars.len();
        let bar_width = screen_width() / n as f32;
        for i in 0..n {
            let i_f = i as f32;
            let height = bars[i].min(screen_height());
            let x = i_f * bar_width;
            let the_color = give_me_the_color(i_f, height, n as f32, screen_height(), &config);
            draw_rectangle(x, screen_height() - height, bar_width, height, the_color);
        }
        bars
    }
}
fn give_me_the_color(index: f32,height : f32 , width_total : f32 , height_total : f32 , config: &Config) -> Color{
    let interpolation_func = |start : f32 , end : f32  , i:f32, total:f32| start + (end-start)* i / total;
    let r = interpolation_func(interpolation_func(config.color_start[0], config.color_end[0], index, width_total), config.color_final[0], height, height_total);
    let g = interpolation_func(interpolation_func(config.color_start[1], config.color_end[1], index, width_total), config.color_final[1], height, height_total);
    let b = interpolation_func(interpolation_func(config.color_start[2], config.color_end[2], index, width_total), config.color_final[2], height, height_total);

    let the_color = Color::new(r,g,b,1.);
    the_color
}