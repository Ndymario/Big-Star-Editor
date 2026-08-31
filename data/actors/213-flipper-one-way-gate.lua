-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 213,
    name = "Flipper (one-way gate)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = -39, width = 16, height = 39 },
    renderBounds = { x = 0, y = -39, width = 16, height = 39 },
    sourceFiles = {
        "/enemy/W_ben.nsbmd ([SOMETHING]_[SOMETHING])",
        "/enemy/W_ben01.nsbmd ([SOMETHING]_[SOMETHING]_01)",
    },
    fields = {
        {
            key = "type",
            label = "Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Nybble 8+ is 2 tiles higher opens from left, and can't be touched by Mario.",
            choices = {
                { value = 0, label = "Faces up; opens from left" },
                { value = 1, label = "Faces up; opens from right" },
                { value = 2, label = "Faces down; opens from left" },
                { value = 3, label = "Faces down; opens from right" },
                { value = 4, label = "Faces right; opens from bottom" },
                { value = 5, label = "Faces right; opens from top" },
                { value = 6, label = "Faces left; opens from bottom" },
                { value = 7, label = "Faces left; opens from top" },
            },
        },
    },
    model = readModel("W_ben.nsbmd"),
}
