-- BG_nsc/d_2d_I_M_back_yakata_boss_nsc.bin
-- StageLayout::fgAnmYakataBoss, which writes into the foreground's tiles: the two layers share one block of them.
return {
    layer = "background",
    id = 49,
    name = "Boss battle",
    tiles = {
        file = "BG_ncg/d_2d_TEN_I_yakata_boss_ncg.bin",
        interval = 5,
        frames = { 0, 4, 8, 12, 64, 68, 72, 76 },
        copies = {
            { layer = "foreground", to = 256, count = 4, from = 0 },
            { layer = "foreground", to = 288, count = 4, from = 16 },
            { layer = "foreground", to = 260, count = 4, from = 32 },
            { layer = "foreground", to = 292, count = 4, from = 48 },
        },
    },
}
