# PdfCraft for Android

The desktop/web egui app hosted in a `NativeActivity`.

- `src/lib.rs`: `android_main`, the Rust entry point.
- `android/`: Gradle project with `MainActivity.kt` (system file picker, "Open with", share target).
- `crates/rfd-shim`, `crates/arboard-shim`: keep `pdfcraft-ui-egui` unchanged on Android, where `rfd` and `arboard` have no backend.
- `.github/workflows/android.yml`: builds the APK on GitHub (Actions ▸ android ▸ Run workflow).

Local build (needs the Android SDK/NDK, JDK 17, `cargo install cargo-ndk`):

```
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo ndk -t arm64-v8a -t armeabi-v7a -o apps/pdfcraft-android/android/app/src/main/jniLibs build --release -p pdfcraft-android
cd apps/pdfcraft-android/android && gradle :app:assembleRelease
```

The APK is universal: it carries arm64-v8a and armeabi-v7a (32-bit phones such as the Galaxy A11), and Android installs the right one.

Known limits of this first version: Save goes to the app's Documents folder (no system "save as" yet), folder pickers and the clipboard are unsupported, and the interface is the desktop layout (touch-friendly layout is still to do).
