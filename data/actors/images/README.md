# Actor stills

These are NSMB Editor 5's `NSMBe5/Resources/Sprite Images`, copied verbatim and
lowercased to `.png`, plus `sprite_invalid.png`, `tileoverrides.png` and
`Icons/arrow_down.png` from the same repository's `Resources` directory.

Most NSMB actors are 3D models that the editor never renders as geometry; NSMBe
draws them as pre-rendered stills instead, and so does Big Star. Actors whose
graphics really are 2D tiles -- pipes, and the tile-based enemies -- load from
the ROM through `ctx.loadBitmap` rather than from here.

Upstream: https://github.com/Dirbaio/NSMB-Editor, GPL-3.0-or-later. Big Star is
GPL-3.0, so the copy is license-compatible; see `docs/licenses.md`.

Nintendo ROM data is not included here and never should be: every file in this
directory is NSMBe editor artwork.
