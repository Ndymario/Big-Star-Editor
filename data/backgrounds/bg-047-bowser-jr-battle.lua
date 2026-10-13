-- BG_nsc/d_2d_I_M_back_toride_boss_nsc.bin
-- StageLayout::fgAnmTorideBoss.
return {
    layer = "background",
    id = 47,
    name = "Bowser Jr battle",
    tiles = {
        file = "BG_ncg/d_2d_TEN_I_toride_boss_ncg.bin",
        interval = 5,
        frames = { 0, 4, 8, 12, 96, 100, 104, 108 },
        copies = {
            { layer = "background", to = 276, count = 4, from = 0 },
            { layer = "background", to = 308, count = 4, from = 16 },
            { layer = "background", to = 280, count = 4, from = 32 },
            { layer = "background", to = 312, count = 4, from = 48 },
            { layer = "background", to = 284, count = 4, from = 64 },
            { layer = "background", to = 316, count = 4, from = 80 },
        },
    },
}
