-- BG_nsc/d_2d_I_M_back_koopa_boss_nsc.bin
-- StageLayout::fgAnmKoopaBoss.
return {
    layer = "background",
    id = 55,
    name = "Final boss battle",
    tiles = {
        file = "BG_ncg/d_2d_TEN_I_koopa_boss_ncg.bin",
        interval = 5,
        frames = { 0, 4, 8, 12, 80, 84, 88, 92 },
        copies = {
            { layer = "background", to = 0, count = 4, from = 0 },
            { layer = "background", to = 32, count = 4, from = 16 },
            { layer = "background", to = 64, count = 4, from = 32 },
            { layer = "background", to = 96, count = 4, from = 48 },
            { layer = "background", to = 128, count = 4, from = 64 },
        },
    },
}
