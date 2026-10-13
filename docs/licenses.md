# License considerations

This repository is currently GPL-3.0 because the root `LICENSE` is GPL version 3. New code in this milestone is written from scratch for this repository.

## Old NSMB Editor

The current NSMB Editor repository contains GPL-3.0-or-later notices, and this repository is GPL-3.0, so the licensing is compatible in both directions.

Three things are taken from it. The `data/actors/*.lua` catalog is adapted from its `stageobjsettings.xml` metadata and the zero-settings behavior of `NSMBStageObj.GetGrabBounds()`. Individual actors' drawing is ported from `NSMBStageObj.cs`, one case at a time, with the upstream case named in each file's header comment. `data/actors/images` is a verbatim copy of `NSMBe5/Resources/Sprite Images` plus `sprite_invalid.png`, `tileoverrides.png` and `Icons/arrow_down.png`.

Copying those stills reverses an earlier decision to exclude them. Most NSMB actors are 3D models that neither editor renders as geometry, so the stills are the only way to draw them at all; they are NSMBe's own artwork under the same license, not Nintendo ROM data. No ROM data is vendored here.

## Interface font, icons and controls

`crates/ui-egui/assets/fonts/NotoSans-Regular.ttf` is Noto Sans, under the SIL Open Font License 1.1; its text is beside it as `OFL.txt`. The icons in `crates/ui-egui/src/icons.rs` are the SVG path data of Heroicons and Feather, both MIT.

The theme, the control set and the motion helpers (`theme.rs`, `controls.rs`, `dialogs.rs`, `motion.rs`, `icons.rs`) are adapted from the Rust frontend of Benkie, by the same author, who licenses that code here under this repository's GPL-3.0.

## NCPatcher

NCPatcher is an external C++ tool. M0 does not vendor it. The integration is a narrow external-process boundary that expects a user-installed `ncpatcher` executable and a project-local `ncpatcher.json`.

## NSMB Code Reference and Code Template

These repositories are references for symbols and workflows. They can involve SDK/toolchain assumptions and generated project files. Do not copy Nintendo SDK headers, Nitro SDK contents, or copyrighted game data. If future work copies template text or code, verify the license first and document attribution.

## Nintendo ROM data and SDK content

No Nintendo ROM, SDK, Nitro SDK, extracted assets, or game data may be committed. Tests must use synthetic fixtures only.

## Future dependencies

Before adding third-party code under `third_party/`, document the license, source URL, version/commit, why vendoring is necessary, update policy, and compatibility with GPL-3.0 distribution.
