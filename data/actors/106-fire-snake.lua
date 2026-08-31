-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 106,
    name = "Fire Snake",
    category = "Enemies - Desert",
    color = "#c9833b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A snake made of fire that follows you by jumping. It's a very annoying enemy!",
    grabBounds = { x = 0, y = -7, width = 27, height = 23 },
    renderBounds = { x = 0, y = -7, width = 27, height = 23 },
    sourceFiles = {
        "/enemy/f_body.nsbmd",
        "/enemy/f_snake.nsbmd",
    },
    fields = {
        {
            key = "jump",
            label = "jump",
            bitOffset = 40,
            bitWidth = 4,
            notes = "do not use on solid walls, game error",
        },
    },
    model = readModel("f_body.nsbmd"),
}
