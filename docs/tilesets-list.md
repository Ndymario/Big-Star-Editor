# The tileset list

The **Tilesets** tab, beside Levels, lists everything the tileset workspace can edit without a level open:

- **Tilesets**: the shared tileset every level loads first (jyotyu), the sub-unit every level loads last (nohara), and
  the 76 level tilesets of the game's tables.
- **Backgrounds** and **Foregrounds**: the 76 of each in the game's tables.
- **Scenes**: layers the game puts together outside a level. The title screen is the only one.

Edit (or a double-click) opens an entry in the workspace with only the tabs it has: a tileset gets Graphics, Map16,
Objects and Behaviors, a background or foreground gets Graphics and its layer, and a scene gets a tab for each of its
layers. Saving writes the files that changed into their modules, as it does for a level's workspace, and edits no
level. A slot whose tables hold no file is listed as empty and cannot be opened.

What each entry is called comes from a YAML list. The editor ships one, `crates/project/tilesets/tilesets.yaml`, and a
project can lay its own `tilesets.yaml`, in the project's root, over it. The same names are what the Course tab offers
when a level picks its tileset, background and foreground.

## The file

```yaml
tilesets:
  - id: 23
    name: "Haunted mansion"
  - name: "Credits tileset"
    graphics: z_new/credits_ncg.bin
    palette: z_new/credits_ncl.bin
    map16: z_new/credits_pnl.bin
    objects: z_new/credits_unt.bin
    object_index: z_new/credits_unt_hd.bin
    behaviors: z_new/credits_chk.bin   # optional
backgrounds:
  - id: 12
    name: "Mushroom plains"
  - name: "Credits sky"
    graphics: z_new/sky_ncg.bin
    palette: z_new/sky_ncl.bin
    screen: z_new/sky_nsc.bin
foregrounds:
  - id: 0
    name: "Rolling hills"
scenes:
  - name: "Title screen"
    background: { graphics: …, palette: …, screen: … }
    tileset: { graphics: …, palette: …, screen: … }
    foreground: { graphics: …, palette: …, screen: … }
```

An entry is one of two kinds:

- **A slot**, with an `id` from 0 to 75. The game's tables in overlay 0 say which files it loads, so it names none of
  its own; a hack that points a table elsewhere opens its own files there.
- **Files**, without an `id`, named by their ROM paths. A tileset names its graphics, palette, Map16, objects and object
  index, and may name its behaviours; a background or foreground names its graphics, palette and screen. Paths, unlike
  NSMB Editor's file IDs, still point at the same files after files are added to the ROM.

A scene names any of its three layers by their files. Each layer is loaded where the game loads that BG, so none of
them has offsets to give: the background's screens number tiles from 576 and palettes from 10, the foreground's from
256 and 8, and the tileset layer's from 192 and 2.

## How a project's list merges

A project's `tilesets.yaml` holds only what it changes:

- a slot it names takes the name it gives;
- an entry or a scene with the name of one in the editor's list takes that one's place, files and all;
- anything else is added after the editor's entries.

Slots stay in table order whatever order the file writes them in. A file that names a slot and files in one entry, a
slot past 75, a path that leaves the ROM, an entry listed twice or a field the list does not know is refused, and the
editor's own list is used instead, with the reason shown at the top of the Tilesets tab.

## The title screen

The title screen, and the opening cutscene that plays in front of the same castle, is drawn by
`Titlescreen_LoadTilesetGraphics` (`021b2b90` in the US ROM), from `TitleScreenScene_setGFXModes`. It loads three
layers by file ID, none of which any table of overlay 0 holds:

| Layer | Files | Loaded at |
| --- | --- | --- |
| BG1, the background | `BG_ncg/d_2d_I_M_back_demo_castle_ncg.bin`, `BG_ncl/d_2d_I_M_back_demo_castle_ncl.bin`, `BG_nsc/d_2d_I_M_back_demo_castle_nsc.bin` | tile 576, palette 10 |
| BG2, the tileset layer | `BG_ncg/d_2d_I_M_tikei_demo_castle_ncg.bin`, `BG_ncl/d_2d_I_M_tikei_demo_castle_ncl.bin`, `BG_nsc/d_2d_I_M_tikei_demo_castle_nsc.bin` | tile 192, palette 2 |
| BG3, the foreground | `BG_ncg/d_2d_I_M_free_demo_castle_ncg.bin`, `BG_ncl/d_2d_I_M_free_demo_castle_ncl.bin`, `BG_nsc/d_2d_I_M_free_demo_castle_nsc.bin` | tile 256, palette 8 |

The BG2 layer is where a level's tileset goes (`tikei` is terrain), but it is drawn from a screen like a background
rather than from Map16 blocks, and has no objects or behaviours. NSMB Editor listed it as a "mapped tileset", with the
other two layers as a 77th background and foreground; here the three are one scene, as the game loads them.

The only other complete layer no table holds is `d_2d_W_M_back_jungle2` (files 357, 519 and 650 of the US ROM), a
background no level can load. It is listed as "Jungle 2 [unused]". Every other BG file the tables miss belongs to the
shared or sub-unit tileset, is a tile animation (`TEN_*`), or is a palette variant of the shared tileset.
