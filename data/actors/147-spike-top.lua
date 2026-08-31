-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 147,
    name = "Spike Top",
    category = "Enemies - Underground",
    color = "#686d76",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A beetle with a spike on its head, that crawls slowly on all sides of a surface.\n\nUnused behaviours:\n- Can be defeated by Drill Stomp [may be spiny leftover?]",
    grabBounds = { x = 0, y = -4, width = 16, height = 20 },
    renderBounds = { x = 0, y = -4, width = 16, height = 20 },
    sourceFiles = {
        "/enemy/toge_met_body.nsbmd",
        "toge means horn (as in spike on an animal)",
    },
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 8,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
                { value = 16, label = "Up (spike on left)" },
                { value = 17, label = "Down (spike on right)" },
                { value = 32, label = "Left (upside down)" },
                { value = 33, label = "Right (upside down)" },
                { value = 48, label = "Down (spike on left)" },
                { value = 49, label = "Up (spike on right)" },
            },
        },
    },
    model = readModel("toge_met_body.nsbmd"),
}
