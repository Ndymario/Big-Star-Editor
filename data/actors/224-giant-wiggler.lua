-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 224,
    name = "Giant Wiggler",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = false,
    notes = "Todo: test path values and other nybbles",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "path_id",
            label = "Path ID",
            bitOffset = 0,
            bitWidth = 4,
            notes = "make paths straight and to a view edge block, or the sprite will not appear",
        },
    },
}
