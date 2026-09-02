-- The end-of-level tower.
--
-- Unlike the pipes, these blocks keep their own Map16: `handle3DTile` records the
-- 3D tile and returns the block unchanged, so the tile draws and the quad goes
-- over it.

return {
    name = "vanilla.tower",
    replaces = false,
    blocks = {
        [1456] = { "polygon_unit/I_toride2_1.nsbtx", "I_toride2_1" },
        [1457] = { "polygon_unit/I_toride2_2.nsbtx", "I_toride2_2" },
        [1458] = { "polygon_unit/I_toride2_3.nsbtx", "I_toride2_3" },
        [1459] = { "polygon_unit/I_toride2_4.nsbtx", "I_toride2_4" },
        [1460] = { "polygon_unit/I_toride2_5.nsbtx", "I_toride2_5" },
        [1461] = { "polygon_unit/I_toride2_6.nsbtx", "I_toride2_6" },
        [1462] = { "polygon_unit/I_toride2_7.nsbtx", "I_toride2_7" },
        [1463] = { "polygon_unit/I_toride2_8.nsbtx", "I_toride2_8" },
        [1464] = { "polygon_unit/I_toride2_9.nsbtx", "I_toride2_9" },
        [1465] = { "polygon_unit/I_toride2_10.nsbtx", "I_toride2_10" },
        [1466] = { "polygon_unit/I_toride2_11.nsbtx", "I_toride2_11" },
        [1467] = { "polygon_unit/I_toride2_12.nsbtx", "I_toride2_12" },
        [1520] = { "polygon_unit/I_toride1.nsbtx", "I_toride1" },
        [1521] = { "polygon_unit/I_toride2.nsbtx", "I_toride2" },
        [1522] = { "polygon_unit/I_toride3.nsbtx", "I_toride3" },
        [1523] = { "polygon_unit/I_toride4.nsbtx", "I_toride4" },
        [1524] = { "polygon_unit/I_toride5.nsbtx", "I_toride5" },
        [1525] = { "polygon_unit/I_toride6.nsbtx", "I_toride6" },
        [1526] = { "polygon_unit/I_toride7.nsbtx", "I_toride7" },
        [1527] = { "polygon_unit/I_toride8.nsbtx", "I_toride8" },
        [1528] = { "polygon_unit/I_toride9.nsbtx", "I_toride9" },
    },
}
