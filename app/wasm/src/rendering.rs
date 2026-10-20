//! The keys the box keeps rendered blocks under, named by the same core the box runs.

use wasm_bindgen::prelude::*;

/// The answer function's key at `source`, as the box prints it beside the block's HTML.
#[wasm_bindgen]
#[must_use]
pub fn answer_key(source: &str) -> String {
    suzatary_core::rendering::key(&suzatary_core::rendering::ANSWER, source)
}

/// The names of the functions a device keeps blocks of, as JSON `[<name>, …]`: a block under any
/// other was named by another build, and goes.
#[wasm_bindgen]
pub fn kept_functions() -> Result<String, JsError> {
    crate::json(
        &suzatary_core::rendering::kept()
            .map(prodrome_core::event::Hash::as_str)
            .to_vec(),
    )
}
