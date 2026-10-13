-- BG_nsc/d_2d_I_M_back_toride_UR_nsc.bin
-- StageLayout::fgAnmYakata2, which writes into the foreground's tiles: the two layers share one block of them.
return {
    layer = "background",
    id = 32,
    name = "Castle 2",
    tiles = {
        file = "BG_ncg/d_2d_TEN_I_toride_ncg.bin",
        interval = 5,
        frames = { 40, 44, 48, 52, 160, 164, 168, 172 },
        copies = {
            { layer = "foreground", to = 288, count = 4, from = 0 },
            { layer = "foreground", to = 292, count = 4, from = 16 },
            { layer = "foreground", to = 296, count = 4, from = 32 },
        },
    },
}
