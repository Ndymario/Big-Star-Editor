-- BG_nsc/d_2d_I_M_free_nohara_W1_1_nsc.bin
-- StageLayout::bgAnmNoharaW1.
return {
    layer = "foreground",
    id = 8,
    name = "Grassland (Peach Castle)",
    tiles = {
        file = "BG_ncg/d_2d_TEN_I_noharaW1_1_ncg.bin",
        interval = 7,
        frames = { 0, 6, 32, 38 },
        copies = {
            { layer = "foreground", to = 256, count = 6, from = 0 },
            { layer = "foreground", to = 288, count = 6, from = 16 },
        },
    },
}
