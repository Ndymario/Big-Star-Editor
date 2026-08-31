-- Ported from NSMB Editor 5's NSMBStageObj.cs case 286.

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2 = RenderX
    local width, height = 0, 0
    local drawn, customRendered = false, true
    customRendered = false
    RenderX2 = (d[5] & 0xF0) - 16
    if RenderX2 ~= 0 then
        drawn = renderer:fillRect(gfx.colored({ x = RenderX + 6, y = RenderY - RenderX2, width = 4, height = RenderX2 + 16 }, "#000000")) or drawn
        drawn = renderer:fillRect(gfx.colored({ x = RenderX, y = RenderY - RenderX2, width = 16, height = 4 }, "#000000")) or drawn
        drawn = renderer:fillRect(gfx.colored({ x = RenderX + 7, y = RenderY - RenderX2 + 1, width = 2, height = RenderX2 + 14 }, "#ffffff")) or drawn
        drawn = renderer:fillRect(gfx.colored({ x = RenderX + 1, y = RenderY - RenderX2 + 1, width = 14, height = 2 }, "#ffffff")) or drawn
    end
    customRendered = false
    return drawn and customRendered
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
