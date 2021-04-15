
#![feature(test)]

pub mod language;
pub mod document;
pub mod oops;

use wasm_bindgen::prelude::*;

pub fn initialize() {
    set_panic_hook();
}

#[wasm_bindgen]
pub fn dbl(x: f64) -> f64 {
    return x * 2.0;
}

#[allow(dead_code)]
pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}