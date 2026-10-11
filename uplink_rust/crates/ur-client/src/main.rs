#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    ur_client::run_native()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
