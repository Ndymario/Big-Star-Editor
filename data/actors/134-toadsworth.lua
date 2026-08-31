-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 134,
    name = "Toadsworth",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = -15, width = 20, height = 31 },
    renderBounds = { x = 0, y = -15, width = 20, height = 31 },
    sourceFiles = {
        "/enemy/kinoji.nsbmd",
    },
    fields = {
        {
            key = "type",
            label = "Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1 makes glitchy blocks that do nothing. Never use it.",
            choices = {
                { value = 0, label = "1UP" },
                { value = 1, label = "Enemy" },
                { value = 2, label = "Item" },
                { value = 3, label = "Mega mushroom" },
                { value = 4, label = "Background chooser" },
            },
        },
    },
    model = readModel("kinoji.nsbmd"),
}
