-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 76,
    name = "Big Unagi",
    category = "Enemies - Water",
    fields = {
        {
            key = "visible_facing_right",
            label = "Visible facing right",
            bitOffset = 16,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Required, or he will not move.",
            valueOffset = 1,
        },
        {
            key = "visible_facing_left",
            label = "Visible facing left",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
