-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 295,
    name = "Fire FG effect 2",
    category = "Effects and decorations",
    notes = "Has the same data as sprites 311, 313, 314, 316, 317, 318, 320, 321 and 322.",
    fields = {
        {
            key = "swaying_start_position",
            label = "Swaying start position",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Also doubles as a direction value (Modulo 4) if nybble 10 and/or nybble 11 are not null. 0 is left-down, 1 is left-up, 2 is right-down and 3 is right-up.",
        },
        {
            key = "swaying_distance",
            label = "Swaying distance",
            bitOffset = 24,
            bitWidth = 4,
            notes = "0 is static.",
        },
        {
            key = "swaying_speed",
            label = "Swaying speed",
            bitOffset = 20,
            bitWidth = 4,
            notes = "0 is static, 15 is the fastest.",
        },
        {
            key = "mixed_shift",
            label = "Mixed shift",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Shift horizontally and vertically.",
        },
        {
            key = "horizontal_scrolling",
            label = "Horizontal scrolling",
            bitOffset = 12,
            bitWidth = 4,
            notes = "1 is the slowest, 16 is the fastest.",
            valueOffset = 1,
        },
        {
            key = "vertical_scrolling",
            label = "Vertical scrolling",
            bitOffset = 8,
            bitWidth = 4,
            notes = "1 is the slowest, 16 is the fastest.",
            valueOffset = 1,
        },
        {
            key = "horizontal_speed",
            label = "Horizontal speed",
            bitOffset = 4,
            bitWidth = 4,
            notes = "1 is the slowest, 16 is the fastest.",
            valueOffset = 1,
        },
        {
            key = "vertical_speed",
            label = "Vertical speed",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1 is the slowest, 16 is the fastest.",
            valueOffset = 1,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
