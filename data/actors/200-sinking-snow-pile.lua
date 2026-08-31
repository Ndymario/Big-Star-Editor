-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 200,
    name = "Sinking-snow pile",
    category = "Enemies - Snow",
    color = "#65a9c7",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Length of 0 makes the game crash!",
    grabBounds = { x = -16, y = 0, width = 48, height = 35 },
    renderBounds = { x = -16, y = 0, width = 48, height = 35 },
    sourceFiles = {
        "/enemy/I_yuki_yuka.nsbtx",
    },
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Game will crash if set to 0!",
        },
    },
    model = readModel("I_yuki_yuka.nsbtx"),
}
