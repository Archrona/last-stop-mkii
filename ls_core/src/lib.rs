mod utils;
pub mod ts_interface;

use wasm_bindgen::prelude::*;

pub fn initialize() {
    utils::set_panic_hook();
}

#[wasm_bindgen]
pub fn dbl(x: f64) -> f64 {
    return x * 2.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dbl() {
        assert_eq!(dbl(10.0), 20.0);
        assert_eq!(dbl(-6.0), -12.0);
    }
}