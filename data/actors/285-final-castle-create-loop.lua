-- Ported from NSMB Editor 5's NSMBStageObj.cs case 285.

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2 = RenderX
    local width, height = 0, 0
    local drawn, customRendered = false, true
    customRendered = false
    RenderX2 = math.max(0, ((d[4] >> 4) + ((d[4] & 0xF) << 4) - 15) * 16)
    if RenderX2 ~= 0 then
        drawn = renderer:fillRect(gfx.colored({ x = RenderX, y = RenderY + 6, width = RenderX2, height = 4 }, "#000000")) or drawn
        drawn = renderer:fillRect(gfx.colored({ x = RenderX + RenderX2 - 4, y = RenderY, width = 4, height = 10 }, "#000000")) or drawn
        drawn = renderer:fillRect(gfx.colored({ x = RenderX + 1, y = RenderY + 7, width = RenderX2 - 2, height = 2 }, "#ffffff")) or drawn
        drawn = renderer:fillRect(gfx.colored({ x = RenderX + RenderX2 - 3, y = RenderY + 1, width = 2, height = 8 }, "#ffffff")) or drawn
    end
    customRendered = false
    return drawn and customRendered
end

return {
    objectId = 285,
    name = "Final Castle Create Loop",
    category = "Other",
    fields = {
        {
            key = "id",
            label = "ID",
            bitOffset = 0,
            bitWidth = 8,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Actual width is found by reversing the nybbles and subtracting 15",
        },
    },
    render = render,
}
