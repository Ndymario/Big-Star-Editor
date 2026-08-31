-- Ported from NSMB Editor 5's NSMBStageObj.cs case 286.

local function render(actor, renderer)
    local pathHeight = (actor.fields.height - 1) * 16
    if pathHeight == 0 then return false end

    local x, top = actor.pixelX, actor.pixelY - pathHeight
    renderer:fillRect({ x = x + 6, y = top, width = 4, height = pathHeight + 16, color = "#000000" })
    renderer:fillRect({ x = x, y = top, width = 16, height = 4, color = "#000000" })
    renderer:fillRect({ x = x + 7, y = top + 1, width = 2, height = pathHeight + 14, color = "#ffffff" })
    renderer:fillRect({ x = x + 1, y = top + 1, width = 14, height = 2, color = "#ffffff" })
    return false
end

return {
    objectId = 286,
    name = "Final Castle Wrong Path",
    category = "Other",
    fields = {
        {
            key = "id",
            label = "ID",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Sprite is positioned at the bottom",
        },
    },
    render = render,
}
