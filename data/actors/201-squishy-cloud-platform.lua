-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 201,
    name = "Squishy cloud platform",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "This squishes under Mario's feet.\n\nThere is an unused (except off screen) tile version in the tileset \"Sky (World 7)\". Apparently has a unseen graphic where all of it  is squished. Only viewable with a Mega Mushroom, though.",
    grabBounds = { x = 0, y = 0, width = 64, height = 32 },
    renderBounds = { x = 0, y = 0, width = 64, height = 32 },
    sourceFiles = {
        "/enemy/l_kumo.nsbtx",
    },
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Cloud extends from sprite position to the right.",
        },
    },
    model = readModel("l_kumo.nsbtx"),
}
