-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 123,
    name = "Roof Spiny",
    category = "Enemies - Underground",
    color = "#686d76",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    sourceFiles = {
        "/enemy/paipo.nsbmd",
        "/enemy/togezo.nsbmd",
        "/enemy/togezo_shell.nsbmd",
    },
    fields = {
        {
            key = "faster",
            label = "Faster",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Unused. Likely a copypaste from the Spiny.",
            checkboxValue = 1,
        },
    },
    model = readModel("paipo.nsbmd"),
}
