-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 89,
    name = "Boomerang Bro.",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = -18, width = 18, height = 34 },
    renderBounds = { x = 0, y = -18, width = 18, height = 34 },
    sourceFiles = {
        "/enemy/hammer_bros.nsbmd",
        "/enemy/boomerang.nsbmd",
    },
    model = readModel("hammer_bros.nsbmd"),
}
