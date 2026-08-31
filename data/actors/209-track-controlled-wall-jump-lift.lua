-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 209,
    name = "Track-controlled wall-jump lift",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -24, y = 5, width = 64, height = 83 },
    renderBounds = { x = -24, y = 5, width = 64, height = 83 },
    sourceFiles = {
        "/enemy/obj_lift_block.nsbtx (object_lift_block)",
    },
    fields = {
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "60 pixels/sec" },
                { value = 1, label = "90 pixels/sec" },
                { value = 2, label = "120 pixels/sec" },
                { value = 3, label = "180 pixels/sec" },
            },
        },
        {
            key = "movement_trigger",
            label = "Movement trigger",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "On its own" },
                { value = 1, label = "When touched at least once" },
            },
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Width is 4+(N11 mod 4)",
            valueOffset = 4,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Height is 4+(N8 mod 4)",
            valueOffset = 4,
        },
    },
    model = readModel("obj_lift_block.nsbtx"),
}
