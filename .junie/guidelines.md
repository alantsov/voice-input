# Project Development Guidelines (Advanced)

This document captures project-specific knowledge to streamline development, testing, and packaging of the voice_input application.

Last verified on: 2025-08-18

## 1) Build and Configuration

Project type: Rust binary with optional features.

Features (from Cargo.toml):
- tray-icon: enables GTK-based tray UI (gtk, libappindicator, glib). Default build excludes this.
- cuda: enables GPU acceleration in whisper-rs. Requires CUDA toolchain and libs present at link time.

Rust toolchain:
- CI uses Rust 1.88.0 (release workflow) and stable (CI matrix). Recommend 1.88.0+.

General builds:
- CPU-only, no tray: cargo build
- With tray icon (GTK): cargo build --features tray-icon
- With CUDA: cargo build --features cuda
- With both: cargo build --features "tray-icon cuda"

Notes on CUDA build:
- build.rs adds link search paths and links CUDA libs when the cuda feature is enabled.
- You must have CUDA libs installed and discoverable. Typical packages on Ubuntu: nvidia-cuda-toolkit nvidia-cuda-dev libcublas-dev (see CI). On some systems CUDA installs to /usr/local/cuda or /opt/cuda.
- LIBCLANG_PATH must be set for bindgen-dependent crates used in the dependency tree. CI sets it via: export LIBCLANG_PATH="$(llvm-config --libdir)"

Notes on tray-icon build:
- Requires GTK and AppIndicator dev packages. On Ubuntu 22.04+: libgtk-3-dev libayatana-appindicator3-dev plus typical X11/xcb dev packages (see README and CI for full list).
- gtk::main() is only invoked when the tray-icon feature is enabled (see src/main.rs #[cfg(feature = "tray-icon")] ).

Runtime data and configuration:
- Config directory (XDG): ~/.config/voice_input/
  - File: config.json with fields: selected_model ("base" by default), translate (bool, default false). See src/config.rs.
- Data directory (XDG): ~/.local/share/voice_input/
  - Models subdir: ~/.local/share/voice_input/models/
  - At startup, transcriber_utils::download_base_models() ensures ggml-base(.en).bin are present; models may also be picked from CWD for backward compatibility (config::get_model_path checks both XDG and current dir).

Binary name and run:
- Binary is voice_input. Run with features as needed, e.g.: cargo run --features "tray-icon" or cargo run --features "tray-icon cuda".

## 2) Packaging (Debian .deb)

Two supported paths:

A) Scripted build:
- ./build_deb.sh
- Uses dpkg-buildpackage -us -uc -b -d (the -d flag skips dependency checks, assuming CI environment prepared deps).
- On success, the .deb lands in the parent directory as ../voice-input_*_amd64.deb.

B) Manual:
- dpkg-buildpackage -us -uc -b    # drop -d if you want dpkg to verify deps

Debian rules specifics (debian/rules):
- Always builds with CUDA and tray-enabled UI: cargo build --release --features tray-icon --features cuda
- Sets CARGO_FEATURE_CUDA=1 in the environment to ensure CUDA paths are configured.
- Prepares a CUDA-related directory under ggml/src/ggml-cuda/… prior to build (needed by whisper-rs/ggml CUDA compilation).
- Installs the binary to /usr/bin/voice-input and installs hicolor icons.
- dh_shlibdeps is invoked with --ignore-missing-info -xlibnvidia-compute-565 to avoid certain CUDA dep warnings.

Local packaging prerequisites (Ubuntu-like):
- debhelper dpkg-dev fakeroot dh-make cargo rustc
- build-essential curl git pkg-config cmake
- libssl-dev libasound2-dev
- X11/XCB deps: libx11-dev libxtst-dev libxi-dev libxkbcommon-dev libxcb-shape0-dev libxcb-xfixes0-dev
- GTK tray deps: libgtk-3-dev libayatana-appindicator3-dev
- Clang/LLVM for bindgen: clang llvm-dev libclang-dev and set LIBCLANG_PATH=$(llvm-config --libdir)
- CUDA stack (required by rules): nvidia-cuda-toolkit nvidia-cuda-dev libcublas-dev

Install built package:
- sudo dpkg -i ../voice-input_<version>_amd64.deb
- sudo apt-get -f install  # resolve missing dependencies

GitHub Actions:
- ci.yml builds on ubuntu-22.04 for default and tray-icon feature variants; packaging job builds a .deb with tray+cuda.
- release.yml triggers on tags vX.Y.Z, builds .deb and uploads it to a GitHub Release. It also bumps debian/changelog version inline.

## 3) Testing

Status: The repository contains no tests by default. Tests are feature-agnostic by design and should avoid linking optional system libraries unless explicitly required.

Recommended approach:
- Keep tests independent of the tray-icon and cuda features unless you specifically intend to validate those paths in an environment with GUI/CUDA available.
- Prefer unit tests inside modules (#[cfg(test)] mod tests) and integration tests under tests/.

Running tests (verified):
- cargo test  # runs with default feature set (no tray-icon, no cuda). This was executed successfully during guideline preparation.

Adding a simple integration test (example):
- Create tests/smoke.rs with:
  ---
  #[test]
  fn sanity_addition() {
      assert_eq!(2 + 2, 4);
  }
  ---
- Run: cargo test
- Result should be 1 passed (verified). Remove the file afterwards if it was created only as a demonstration.

Running tests with features:
- cargo test --features tray-icon              # requires GTK dev deps installed
- cargo test --features cuda                   # requires CUDA toolchain and libs
- cargo test --features "tray-icon cuda"       # requires both

Guidelines for writing new tests:
- Do not spin up the full application main loop in tests. Factor logic into testable functions/structs.
- Avoid global state interference (e.g., lazy_static globals). Where unavoidable, ensure isolation or reset behaviors between tests.
- Keep tests deterministic: avoid network downloads (transcriber_utils::download_base_models) and audio capture. If you need model paths, mock config::get_model_path.
- For crate structure, consider extracting non-UI logic into a library module to enable easier unit testing without involving GTK or global listeners.

## 4) Additional Development Notes

Code style and linting:
- Use rustfmt: cargo fmt --all
- Use clippy: cargo clippy --all-targets --all-features -D warnings
- CI toolchains include rustfmt/clippy in release.yml; ci.yml has placeholders to run tests/clippy when desired.

Logging and diagnostics:
- The project primarily uses println!/eprintln! for status and error reporting. Run via cargo run to observe stdout/stderr.
- When tray-icon is enabled, the GTK main loop runs on the main thread; background threads handle keyboard events and app loop. Print diagnostics still appear on the launching terminal.

Keyboard and single-instance behavior:
- Global keyboard events are handled via rdev in a background thread. The trigger is Ctrl+CapsLock to start/stop recording.
- A single-instance guard is established at startup (single_instance::ensure_single_instance) using file locking (via fs2). Tests should not attempt to execute the binary concurrently.

Model selection and translation:
- Model selection is persisted in config.json (selected_model). translate flag toggles translation to English.
- transcriber_utils::select_model_file resolves model filenames based on selected_model and language mode with fallbacks to base models.
- If a requested model is missing, ensure_transcriber_for falls back to base variants and logs errors instead of panicking.

Common pitfalls and fixes:
- Missing LIBCLANG_PATH causes bindgen-related build failures: set LIBCLANG_PATH=$(llvm-config --libdir) and ensure libclang.so is present.
- CUDA linking errors: install CUDA toolkit/dev packages or omit the cuda feature.
- GTK missing headers: install libgtk-3-dev and libayatana-appindicator3-dev when enabling tray-icon.
- Debian packaging forces CUDA+tray; ensure CUDA/GTK deps are installed or modify debian/rules if you intend to build a CPU-only package locally.

## 5) Quick Reference Commands

- CPU build: cargo build
- Tray UI build: cargo build --features tray-icon
- CUDA build: cargo build --features cuda
- Full build: cargo build --features "tray-icon cuda"
- Run (CPU): cargo run
- Run (tray+CUDA): cargo run --features "tray-icon cuda"
- Test (default features): cargo test
- Format & lint: cargo fmt --all && cargo clippy --all-targets --all-features -D warnings
- Package (.deb): ./build_deb.sh

If you encounter environment-related build issues, compare your environment with the CI setup in .github/workflows/*.yml.
