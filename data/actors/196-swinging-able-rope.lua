-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 196,
    name = "Swinging/able rope",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -3, y = 0, width = 8, height = 64 },
    renderBounds = { x = -3, y = 0, width = 8, height = 64 },
    fields = {
        {
            key = "mario_controls_sway",
            label = "Mario controls sway",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "length_in_tiles",
            label = "Length in tiles",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Maximum = 8.",
            valueOffset = 4,
        },
        {
            key = "sway_on_own",
            label = "Sway on own +/-",
            bitOffset = 4,
            bitWidth = 4,
            notes = "These values go from 0 (none) to F (almost horizontal). At a 4 tile (0 value length) they increase 1/2 tile at the bottom per number.",
        },
    },
}
