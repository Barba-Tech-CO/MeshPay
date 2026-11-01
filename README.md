# MeshPay

Phase 0 offline BLE payment prototype: Flutter app (Rust is planned for the backend only).

## Layout
- `app/`: Flutter Android (minSdk 29) and iOS (deployment target 15.0).
- `packages/design_system/`: shared theme and color tokens (IBM Plex through the `google_fonts` package).

## Run
- `cd app && flutter pub get && flutter run`

## Security CI
- Pull requests and pushes to `main` run RustSec, cargo-deny, OSV-Scanner, Gitleaks, Rust, and Flutter checks.
- Dependency policy is defined in `deny.toml`; known-vulnerability exceptions are not allowlisted.
- Run `bash scripts/security/inject-cases.sh` to verify all six security gates with isolated fixtures.
- The Android Gradle lockfile covers only the shipped debug, profile, and release runtime classpaths.
- Gradle tool, lint, unit-test, and instrumentation-test configurations are excluded because they are not packaged in the APK.
- OSV-Scanner therefore checks the Android dependencies that ship to users, not tooling-only configurations.
