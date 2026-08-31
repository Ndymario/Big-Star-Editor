-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 70,
    name = "Sledge Bro.",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Can destroy smashable (question mark/brick/wood/stone/used) blocks.",
    grabBounds = { x = 0, y = -15, width = 26, height = 31 },
    renderBounds = { x = 0, y = -15, width = 26, height = 31 },
    sourceFiles = {
        "/enemy/himan.nsbmd",
    },
    model = readModel("himan.nsbmd"),
}
