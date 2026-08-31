-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 96,
    name = "Climbing Koopa",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 18, height = 26 },
    renderBounds = { x = 0, y = 0, width = 18, height = 26 },
    sourceFiles = {
        "/enemy/net_nokonoko.nsbmd",
    },
    fields = {
        {
            key = "walking_direction",
            label = "Walking direction",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Horizontal" },
                { value = 1, label = "Vertical" },
            },
        },
        {
            key = "fence_position",
            label = "Fence position",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "In front" },
                { value = 1, label = "Behind" },
            },
        },
        {
            key = "faster",
            label = "Faster",
            bitOffset = 12,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "color",
            label = "Color",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Green" },
                { value = 1, label = "Red" },
            },
        },
    },
    model = readModel("net_nokonoko.nsbmd"),
}
