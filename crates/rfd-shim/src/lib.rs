//! `rfd` under another name, so `pdfcraft-ui-egui` keeps calling `rfd::AsyncFileDialog` unchanged.
//!
//! Everywhere `rfd` runs, this crate is `rfd`. Android has no `rfd` backend, so there the same
//! surface is a small stand-in: a dialog writes a request file into a mailbox folder, the Kotlin
//! activity shows the system picker, copies the chosen documents into the app's cache and writes
//! the paths back. No JNI and no `unsafe` on the Rust side (see `apps/pdfcraft-android`).

#[cfg(not(target_os = "android"))]
pub use rfd::*;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::*;
