-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 269,
    name = "Warp Cannon",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Note: \"w_cannon.nsbmd\" cannot be opened with NSMBe, to fix this problem, see this tutorial:\n\nhttp://nsmbhd.net/thread/3250-fixing-already-opened-files-tutorial/",
    grabBounds = { x = -9, y = -26, width = 66, height = 90 },
    renderBounds = { x = -9, y = -26, width = 66, height = 90 },
    sourceFiles = {
        "/enemy/w_cannon.nsbmd",
    },
    fields = {
        {
            key = "destination_world",
            label = "Destination world",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Worlds 1-4 have no icons. Cannon will appear as \"W5\" for them.",
            choices = {
                { value = 1, label = "World 1" },
                { value = 2, label = "World 2" },
                { value = 3, label = "World 3" },
                { value = 4, label = "World 4" },
                { value = 5, label = "World 5" },
                { value = 6, label = "World 6" },
                { value = 7, label = "World 7" },
                { value = 8, label = "World 8" },
            },
        },
    },
    model = readModel("w_cannon.nsbmd"),
}
