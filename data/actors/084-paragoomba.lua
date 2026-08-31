-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 84,
    name = "Paragoomba",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Unused behaviours:\n- Has ceiling detection [may be used]\n- Has water interaction [may be used, or Goomba leftover]",
    grabBounds = { x = 0, y = -12, width = 24, height = 28 },
    renderBounds = { x = 0, y = -12, width = 24, height = 28 },
    sourceFiles = {
        "/obj/I_kuribo256_ncg.bin",
    },
    graphics = readGraphics("I_kuribo256_ncg.bin"),
    palette = readPalette("I_enemy_ncl.bin"),
}
