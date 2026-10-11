//! Ghostline client: one egui codebase for desktop and the browser.

mod app;
pub mod game;
pub mod persist;

pub use app::GhostlineApp;

#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Ghostline")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([980.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Ghostline",
        options,
        Box::new(|cc| Ok(Box::new(GhostlineApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::GhostlineApp;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    /// Called from `index.html` with the id of a `<canvas>`.
    #[wasm_bindgen]
    pub async fn start(canvas_id: String) -> Result<(), JsValue> {
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(&canvas_id))
            .ok_or_else(|| JsValue::from_str("canvas not found"))?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(GhostlineApp::new(cc)))),
            )
            .await
    }
}
