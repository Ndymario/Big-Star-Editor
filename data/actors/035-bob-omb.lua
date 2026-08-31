-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 35,
    name = "Bob-omb",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A walking bomb.",
    grabBounds = { x = 0, y = -2, width = 16, height = 18 },
    renderBounds = { x = 0, y = -2, width = 16, height = 18 },
    sourceFiles = {
        "/enemy/bombhei.nsbca",
        "/enemy/bombhei.nsbmd",
    },
    fields = {
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 28,
            bitWidth = 4,
            notes = "8 is used by ones thrown out of the Monty Tank.",
            choices = {
                { value = 0, label = "Default" },
                { value = 4, label = "Pipe generator" },
                { value = 2, label = "Self-destructs (240 frames)" },
                { value = 8, label = "Self-destructs (241 frames)" },
            },
        },
        {
            key = "pipe_generator_direction",
            label = "Pipe generator direction",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 2, label = "Left" },
                { value = 3, label = "Right" },
            },
        },
    },
    model = readModel("bombhei.nsbmd"),
}
