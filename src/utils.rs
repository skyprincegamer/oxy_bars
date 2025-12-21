use macroquad::color::Color;
use macroquad::prelude::{draw_rectangle, screen_height, screen_width};
use crate::{COLOR_END, COLOR_FINAL, COLOR_START, GLOBAL_SCALE};

pub fn average_the_bars(bars: Vec<f32>, target_size : usize) -> Vec<f32>{

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

pub fn scale_the_bars(bars: Vec<f32>) -> Vec<f32>{
    let mut output = vec![0.; bars.len()];
    let scale = GLOBAL_SCALE * screen_width();
    for i in 0..output.len(){
        output[i] = bars[i] * scale;
    }
    output
}

pub fn interpolate_the_bars(this_bars : Vec<f32> , other : &Vec<f32> , alpha:f32) -> Vec<f32>{
    this_bars.iter().enumerate().map(|(i,&val)|{(val * alpha) + other[i] * (1.-alpha)}).collect()
}
pub fn draw_rectangles(bars : Vec<f32>){
    for i in 0..bars.len() {
        let i_f = i as f32;
        let height = bars[i].min(screen_height());
        let the_color = give_me_the_color(i_f , height , screen_width() , screen_height());
        draw_rectangle(i_f, screen_height() - height, 1. , height , the_color );
    }
}
fn give_me_the_color(index: f32,height : f32 , width_total : f32 , height_total : f32 ) -> Color{
    let interpolation_func = |start : f32 , end : f32  , i:f32, total:f32| start + (end-start)* i / total;
    let r = interpolation_func(interpolation_func(COLOR_START.0 , COLOR_END.0 , index, width_total) , COLOR_FINAL.0 , height , height_total);
    let g = interpolation_func(interpolation_func(COLOR_START.1 , COLOR_END.1 , index, width_total) , COLOR_FINAL.1 , height , height_total);
    let b = interpolation_func(interpolation_func(COLOR_START.2 , COLOR_END.2 , index, width_total) , COLOR_FINAL.2 , height , height_total);

    let the_color = Color::new(r,g,b,1.);
    the_color
}