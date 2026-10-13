-- BG_nsc/d_2d_W_M_free_kazan_nsc.bin
-- StageLayout::bgAnmKazan: 22 steps between its first two frames, 16 between the next two, then the last two held
-- longer. On the final frame the game also spawns its raining debris.
local frames = {}
for step = 0, 21 do frames[#frames + 1] = { (step % 2) * 16, ticks = 10 } end
for step = 0, 15 do frames[#frames + 1] = { (2 + step % 2) * 16, ticks = 10 } end
frames[#frames + 1] = { 4 * 16, ticks = 20 }
frames[#frames + 1] = { 5 * 16, ticks = 48 }
return {
    layer = "foreground",
    id = 56,
    name = "Volcanoes 2 (outside)",
    strips = {
        { throughRow = 304, drift = 0.375 },
        { throughRow = 320, drift = 0.3125 },
        { scroll = 1.0 },
    },
    tiles = {
        file = "BG_ncg/d_2d_TEN_W_kazan_ncg.bin",
        frames = frames,
        copies = {
            { layer = "foreground", to = 304, count = 16, from = 0 },
        },
    },
}
