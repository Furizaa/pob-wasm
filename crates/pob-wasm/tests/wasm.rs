use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn version_is_not_empty() {
    assert!(!pob_wasm::version().is_empty());
}
