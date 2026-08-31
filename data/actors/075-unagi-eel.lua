-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 75,
    name = "Unagi (eel)",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "This sprite will not spawn if none of the values are checked.\nOfficially known as the 'Unagi'.",
    grabBounds = { x = 12, y = 3, width = 104, height = 28 },
    renderBounds = { x = 12, y = 3, width = 104, height = 28 },
    sourceFiles = {
        "/enemy/moray.nsbmd",
    },
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 20,
            notes = "use with the Auto-scrolling controller sprite, to avoid the bug of when the unagi appears as a glich",
            choices = {
                { value = 0, label = "Doesn't spawn" },
                { value = 1, label = "Stays in place" },
                { value = 16, label = "Left" },
                { value = 65536, label = "Right" },
            },
        },
    },
    model = readModel("moray.nsbmd"),
}
