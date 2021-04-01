mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn dbl(x: f64) -> f64 {
    return x * 2.0;
}