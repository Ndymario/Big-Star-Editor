-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 211,
    name = "Smashed pipe sprite",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    notes = "The effect will activate every time you get to the point were you can see the sprite.\nThis sprite gets spawned in-game every time you break a pipe as mega Mario. This is only the case for a certain data.\n\nNOTE: There was sprite data controlling the size, color and behavior. Must investigate.\n\nhttp://board.dirbaio.net/thread.php?id=353 Please check here.",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "color_type",
            label = "Color/type?",
            bitOffset = 20,
            bitWidth = 4,
        },
        {
            key = "facing",
            label = "Facing?",
            bitOffset = 16,
            bitWidth = 4,
            notes = "0-9 destroy?",
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 8,
        },
    },
}
