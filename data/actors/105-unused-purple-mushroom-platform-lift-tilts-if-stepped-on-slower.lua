-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 105,
    name = "Unused Purple Mushroom Platform Lift (tilts if stepped on) (slower)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "Unused. Tilts slower than used purple tilting mushroom. Fits perfectly with the stalk object in the snow with ice tileset (Hiccup: Is this not true for the other one?)",
    grabBounds = { x = -16, y = 0, width = 64, height = 112 },
    renderBounds = { x = -16, y = 0, width = 64, height = 112 },
    sourceFiles = {
        "/enemy/I_kinoko_ashib_s.nsbtx",
    },
    fields = {
        {
            key = "width",
            label = "Width",
            bitOffset = 4,
            bitWidth = 4,
        },
    },
    model = readModel("I_kinoko_ashib_s.nsbtx"),
}
