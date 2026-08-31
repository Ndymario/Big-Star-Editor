-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 56,
    name = "Big Dry Bones",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = -28, width = 25, height = 44 },
    renderBounds = { x = 0, y = -28, width = 25, height = 44 },
    sourceFiles = {
        "/enemy/karon_body.nsbmd",
        "/enemy/karon_head.nsbmd",
    },
    model = readModel("karon_body.nsbmd"),
}
