-- BG_nsc/d_2d_W_M_free_kazangake_nsc.bin
-- StageLayout::bgAnmKazan: 22 steps between its first two frames, 16 between the next two, then the last two held
-- longer. On the final frame the game also spawns its raining debris.
-- No level uses this one. Its frames do not match the tiles they land on, so it garbles them in the game as well.
local frames = {}
for step = 0, 21 do frames[#frames + 1] = { (step % 2) * 16, ticks = 10 } end
for step = 0, 15 do frames[#frames + 1] = { (2 + step % 2) * 16, ticks = 10 } end
frames[#frames + 1] = { 4 * 16, ticks = 20 }
frames[#frames + 1] = { 5 * 16, ticks = 48 }
return {
    layer = "foreground",
    id = 16,
    name = "Volcanoes 3 (outside) [Unused]",
    tiles = {
        file = "BG_ncg/d_2d_TEN_W_kazangake_ncg.bin",
        frames = frames,
        copies = {
            { layer = "foreground", to = 231, count = 16, from = 0 },
        },
    },
}
