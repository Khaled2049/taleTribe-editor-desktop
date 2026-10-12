mod ai;
mod assets;
mod commands;
mod db;
mod error;
mod secrets;
mod store;
mod sync;

use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use tauri::Manager;

use commands::AppState;

pub fn run() {
    tauri::Builder::default()
        // Used only from Rust (`open_website`); the webview is granted none of it.
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let assets_dir = data_dir.join("assets");
            fs::create_dir_all(&assets_dir)?;
            let conn = db::open(&data_dir.join("taletribe.db"))?;
            app.manage(AppState {
                db: Mutex::new(conn),
                assets_dir,
                secrets: Box::new(secrets::Cached::new(secrets::Keychain)),
                http: ai::providers::client(),
                generations: Mutex::new(HashMap::new()),
                config: sync::config::Config::from_build(),
                session: Mutex::new(None),
            });
            Ok(())
        })
        // Images load through this scheme, so the webview gets files from the
        // assets folder only and no general file access.
        .register_asynchronous_uri_scheme_protocol("ttasset", |ctx, request, responder| {
            let dir = ctx.app_handle().state::<AppState>().assets_dir.clone();
            let name = request.uri().path().trim_start_matches('/').to_string();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(assets::serve(&dir, &name));
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_stories,
            commands::create_story,
            commands::update_story,
            commands::delete_story,
            commands::list_chapters,
            commands::get_chapter,
            commands::create_chapter,
            commands::rename_chapter,
            commands::delete_chapter,
            commands::reorder_chapters,
            commands::save_chapter_content,
            commands::list_snapshots,
            commands::get_snapshot_content,
            commands::restore_snapshot,
            commands::list_characters,
            commands::save_character,
            commands::delete_character,
            commands::list_places,
            commands::save_place,
            commands::delete_place,
            commands::import_image,
            commands::get_ai_settings,
            commands::save_ai_settings,
            commands::set_api_key,
            commands::delete_api_key,
            commands::list_models,
            commands::generate,
            commands::cancel_generation,
            commands::get_account,
            commands::sign_in,
            commands::sign_out,
            commands::publish_preview,
            commands::publish,
            commands::open_website,
            commands::list_website_stories,
            commands::import_story,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
