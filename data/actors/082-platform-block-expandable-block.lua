-- Ported from NSMB Editor 5's NSMBStageObj.cs case 82.

local images = {}

local function init(ctx)
    images.ExpandableBlock = ctx.loadImage("images/ExpandableBlock.png")
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.ExpandableBlock, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 82,
    name = "Platform Block (expandable block)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "turn_used_at_4th_hit",
            label = "Turn used at 4th hit",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Turns into used blocks. Unused. All values above 1 seem to produce same result as 1.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
}
