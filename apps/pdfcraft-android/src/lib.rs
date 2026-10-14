//! PdfCraft for Android.
//!
//! The same egui app as the desktop and the web, hosted in a `NativeActivity`. The Kotlin
//! activity (`android/`) does what only Java APIs can: it shows the system document picker and
//! copies the chosen files into the app's cache. It talks to this side through plain files in a
//! mailbox folder (`pdfcraft-rfd` explains the request/result format), and hands over files that
//! other apps open with PdfCraft the same way, in `incoming/`.

#![cfg(target_os = "android")]
#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use pdfcraft_ui_egui::{Inbox, PdfCraftApp};
use winit::platform::android::activity::AndroidApp;

#[unsafe(no_mangle)]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("PdfCraft"));

    let mailbox = app.internal_data_path().map(|d| d.join("mailbox"));
    if let Some(dir) = &mailbox {
        pdfcraft_rfd::set_mailbox(dir.clone());
    }

    let options = eframe::NativeOptions { renderer: eframe::Renderer::Glow, android_app: Some(app), ..Default::default() };

    let result = eframe::run_native(
        "PdfCraft",
        options,
        Box::new(move |cc| {
            let mut app = PdfCraftApp::new();
            if let Some(json) = cc.storage.and_then(|s| s.get_string("pdfcraft")) {
                app.restore(&json);
            }
            if let Some(dir) = mailbox {
                watch_incoming(dir.join("incoming"), app.inbox.clone(), cc.egui_ctx.clone());
            }
            Ok(Box::new(app))
        }),
    );
    if let Err(e) = result {
        log::error!("PdfCraft could not start: {e}");
    }
}

/// Files other apps opened with PdfCraft arrive as small `*.txt` notes holding the path of a
/// copy in the cache. Reads each copy into the app's inbox, like a file picked in the app.
fn watch_incoming(dir: PathBuf, inbox: Inbox, ctx: egui::Context) {
    let _ = std::fs::create_dir_all(&dir);
    std::thread::spawn(move || {
        loop {
            take_incoming(&dir, &inbox, &ctx);
            std::thread::sleep(Duration::from_millis(300));
        }
    });
}

fn take_incoming(dir: &Path, inbox: &Inbox, ctx: &egui::Context) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let note = entry.path();
        if note.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let target = std::fs::read_to_string(&note).ok().map(|t| PathBuf::from(t.trim()));
        let _ = std::fs::remove_file(&note);
        let Some(target) = target else { continue };
        match std::fs::read(&target) {
            Ok(bytes) => {
                let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "document.pdf".into());
                if let Ok(mut q) = inbox.lock() {
                    q.push((name, bytes));
                }
                ctx.request_repaint();
            }
            Err(e) => log::warn!("could not read {}: {e}", target.display()),
        }
    }
}
