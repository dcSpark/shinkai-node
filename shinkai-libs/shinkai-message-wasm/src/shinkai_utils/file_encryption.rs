use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use blake3::Hasher;

#[wasm_bindgen]
pub fn calculate_blake3_hash(input: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result.as_bytes())
}