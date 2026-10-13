-- BG_nsc/d_2d_W_M_back_kazangake_nsc.bin
-- StageLayout::fgAnmKazangake.
return {
    layer = "background",
    id = 16,
    name = "Volcanoes 3 (outside) [Unused]",
    strips = {
        { throughRow = 384, wave = { amplitude = 2.3125, speed = 256, step = 1536 } },
        { scroll = 1.625 },
    },
    tiles = {
        file = "BG_ncg/d_2d_TEN_W_kazangake_ncg.bin",
        interval = 15,
        frames = { 96, 102, 108, 114 },
        copies = {
            { layer = "background", to = 14, count = 6, from = 0 },
        },
    },
}
