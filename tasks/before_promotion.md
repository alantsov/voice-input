# Before Promotion Checklist (voice_input)

This repository is working, published on GitHub, and all planned features are implemented. Use this checklist to polish and validate everything before wider promotion (HN/Reddit, product directories, newsletters, etc.). Keep it practical and quick to execute.

## 1) Quality & Consistency
- Run format and lints (no warnings):
  - cargo fmt --all
  - cargo clippy --all-targets --all-features -D warnings
- Ensure Cargo.toml metadata is complete: authors, repository, homepage, license, categories, keywords, description, readme.
- Verify LICENSE file matches the license declared in Cargo.toml.
- Confirm binary name is voice_input in Cargo.toml and packaging artifacts (voice-input in .deb aligns with desktop file).

## 2) Build Matrix Sanity (local)
- Default CPU build (no optional features): cargo build
- Tray icon only (requires GTK deps): cargo build --features tray-icon
- CUDA only (requires CUDA toolchain): cargo build --features cuda
- Tray + CUDA: cargo build --features "tray-icon cuda"
- If you cannot satisfy GTK/CUDA locally, at least build default locally and rely on CI for feature builds.

## 3) Runtime Smoke
- Run default: cargo run
- Verify:
  - Single-instance guard works (second run should refuse).
  - Ctrl+CapsLock toggles recording; WAV saved with timestamp; transcription inserts text at cursor.
  - Config file created at ~/.config/voice_input/config.json; defaults applied (selected_model, translate).
  - Base models auto-downloaded or discovered in ~/.local/share/voice_input/models/ (or CWD fallback).
- With tray-icon: cargo run --features tray-icon
  - Tray appears; basic menu actions work (quit, settings if present). Logs visible in terminal.
- With CUDA (if available): ensure GPU acceleration path is used without runtime/link errors.

## 4) Packaging (.deb)
- Build Debian package (from repo root):
  - ./build_deb.sh
  - Result: ../voice-input_<version>_amd64.deb
- Install on a clean-ish Ubuntu 22.04+ VM/container:
  - sudo dpkg -i ../voice-input_<version>_amd64.deb
  - sudo apt-get -f install
- Verify:
  - /usr/bin/voice-input exists and launches.
  - Desktop entry and icons installed; tray icon visible with feature build.
  - dh_shlibdeps warnings are handled as in debian/rules.

## 5) CI/CD & Releases
- Ensure GitHub Actions workflows succeed (ci.yml and release.yml):
  - Default build job passes.
  - Tray-icon feature build passes (on ubuntu-22.04 with GTK deps).
  - Packaging job builds .deb with tray+cuda.
- Tag a release candidate and verify release workflow uploads artifacts:
  - git tag vX.Y.Z && git push origin vX.Y.Z
  - Check that debian/changelog was bumped and Release assets (.deb) are attached.

## 6) Documentation & Assets
- README
  - Quick start (install deps, build commands) is accurate for Ubuntu 22.04+.
  - Include notes about optional features: tray-icon, cuda, and their dependencies.
  - Add a short section on keyboard shortcut (Ctrl+CapsLock), config location, and model management.
  - Add screenshots/GIFs: tray icon, example transcription.
- Add CHANGELOG.md highlights for the release.
- Verify voice-input.svg and icons in assets/ are properly referenced (desktop file, README badges/images).
- Confirm voice-input.desktop contents are correct (Name, Exec, Icon, Categories) and align with install paths.

## 7) User Experience Polish
- Sensible defaults in config.json (selected_model="base", translate=false).
- Error handling:
  - Missing model => graceful fallback to base(.en) with clear stderr log.
  - Microphone unavailable => informative error, no panic.
  - CUDA missing while feature enabled => clear link error guidance in README.
- Logging: ensure startup prints helpful environment hints (CUDA/GTK availability, config path).

## 8) Security & Privacy
- Confirm no sensitive tokens or PII are logged.
- Explain briefly in README how/where audio is stored and that transcription runs locally (whisper-rs).
- Ensure dependencies are up-to-date (cargo update; consider cargo deny/audit in CI later).

## 9) Community & Project Hygiene
- Add GitHub templates (optional but helpful):
  - .github/ISSUE_TEMPLATE/bug_report.md, feature_request.md
  - .github/PULL_REQUEST_TEMPLATE.md
- Add CONTRIBUTING.md with build prerequisites and coding standards (fmt/clippy).
- Add CODE_OF_CONDUCT.md (e.g., Contributor Covenant) if community contributions are expected.

## 10) Versioning & Tagging
- Bump version in Cargo.toml and debian/changelog consistently.
- Use Semantic Versioning.
- After tagging vX.Y.Z, ensure the app prints the version (e.g., via env! macro in --version flag or at startup).

## 11) Post-Promotion Checklist
- Monitor GitHub Issues and Discussions for the first week.
- Prepare quick fixes for common environment issues (LIBCLANG_PATH, GTK dev deps, CUDA).
- Consider publishing to additional channels:
  - crates.io (if appropriate; this is a binary but can still be published)
  - Reddit r/rust, r/linux, HN, Mastodon/Bluesky, relevant Discords/Slack.

## Quick Reference Commands
- cargo build                       # CPU-only
- cargo build --features tray-icon
- cargo build --features cuda
- cargo build --features "tray-icon cuda"
- cargo run                         # default
- cargo test                        # default features only
- cargo fmt --all && cargo clippy --all-targets --all-features -D warnings
- ./build_deb.sh                    # build .deb

Notes:
- For feature builds, ensure system deps are installed (see README and CI).
- Set LIBCLANG_PATH to $(llvm-config --libdir) when building with bindgen-dependent crates.
