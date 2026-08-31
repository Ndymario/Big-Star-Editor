-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 133,
    name = "Big Bowser (unused sprite)",
    category = "Bosses and cutscenes",
    notes = "Spawned by Final Bowser controller\nGame crashes after roaring animation if used anywhere other than World 8 Final Castle Area 3. Seems to be a carbon copy of the one seen with Bowser Jr. in World 8 Final Castle Area 3.",
    fields = {
        {
            key = "bridge_id",
            label = "Bridge ID",
            bitOffset = 40,
            bitWidth = 8,
            notes = "Set to 2 when spawned by Final Bowser controller",
        },
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Used when spawned by Final Bowser controller",
            checkboxValue = 1,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
