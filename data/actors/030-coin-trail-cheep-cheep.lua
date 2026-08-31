-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 30,
    name = "Coin trail Cheep Cheep",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -62, y = 4, width = 80, height = 29 },
    renderBounds = { x = -62, y = 4, width = 80, height = 29 },
    sourceFiles = {
        "/enemy/pukupuku.nsbmd",
    },
    fields = {
        {
            key = "swims_left",
            label = "Swims left",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 8,
        },
        {
            key = "avoids_mario",
            label = "Avoids Mario",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Also swims faster. Unused.",
            checkboxValue = 1,
        },
    },
    model = readModel("pukupuku.nsbmd"),
}
