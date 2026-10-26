//! BitStream desktop client: a Tauri shell around the `bs_node` library.
//!
//! The app links `bs-node` as a library and runs at most one node at a time on
//! the Tauri tokio runtime. The node serves the web player on loopback and the
//! window shows that page in a frame.

#![forbid(unsafe_code)]
// WHY: Tauri apps open a window; a console on Windows is noise.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod link;
mod node;

use node::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,quinn=warn".into()),
        )
        .init();
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            node::start_viewer,
            node::start_publisher,
            node::stop,
            node::status,
        ])
        .run(tauri::generate_context!())
        .expect("tauri runtime");
}
