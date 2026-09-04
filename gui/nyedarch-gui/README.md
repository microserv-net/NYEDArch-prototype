# nyedarch-gui

The NYEDArch desktop client (Phase 6). Built with **egui/eframe** so all
security-sensitive logic stays inside Rust the client controls (spec §69).

This crate is intentionally **not** a member of the root workspace: it needs a
display and pulls windowing/GPU dependencies, so it is built separately on a
desktop. The core capsule/crypto crates it drives are fully built and tested in
the workspace.

```bash
cd gui/nyedarch-gui
cargo run --release
```

The UI has an original design language — its own terminology ("capsule",
"protections", "forge"), navigation, and visual hierarchy. Thermite is referenced only
for GitHub build orchestration concepts, never for UI (spec §64/§65).
