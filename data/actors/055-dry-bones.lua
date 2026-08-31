-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 55,
    name = "Dry Bones",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Ignores water.",
    grabBounds = { x = 0, y = -10, width = 17, height = 26 },
    renderBounds = { x = 0, y = -10, width = 17, height = 26 },
    sourceFiles = {
        "/enemy/karon_body.nsbmd",
        "/enemy/karon_head.nsbmd",
    },
    model = readModel("karon_body.nsbmd"),
}
