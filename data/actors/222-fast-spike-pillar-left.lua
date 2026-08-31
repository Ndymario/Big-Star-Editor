-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 222,
    name = "Fast Spike Pillar - Left",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 144, height = 64 },
    renderBounds = { x = 0, y = 0, width = 144, height = 64 },
    sourceFiles = {
        "enemy/I_TEST_kui.nsbmd",
    },
    fields = {
        {
            key = "speed",
            label = "Speed",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 3, label = "Doesn't move" },
                { value = 2, label = "Slowest" },
                { value = 0, label = "Medium" },
                { value = 1, label = "Fastest" },
            },
        },
        {
            key = "start",
            label = "Start",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Forward" },
                { value = 2, label = "Backward" },
                { value = 8, label = "With delay" },
            },
        },
        {
            key = "distance",
            label = "Distance",
            bitOffset = 20,
            bitWidth = 4,
            notes = "Time travel is the same no matter the distance.",
            choices = {
                { value = 1, label = "7 tiles" },
                { value = 3, label = "10 tiles" },
                { value = 2, label = "14 tiles" },
                { value = 0, label = "16 tiles" },
            },
        },
        {
            key = "forward_delay",
            label = "Forward delay",
            bitOffset = 12,
            bitWidth = 8,
            notes = "60 frames = 1 second.",
        },
        {
            key = "backward_delay",
            label = "Backward delay",
            bitOffset = 0,
            bitWidth = 8,
            notes = "60 frames = 1 second.",
        },
    },
    model = readModel("I_TEST_kui.nsbmd"),
}
