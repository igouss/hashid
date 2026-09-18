//! The bindings generator, vendored so it always matches the `uniffi` version
//! the scaffolding in this crate was built with.
//!
//! ```text
//! cargo run --bin uniffi-bindgen -- generate --library <cdylib> --language python --out-dir <dir>
//! ```

fn main() {
    uniffi::uniffi_bindgen_main();
}
