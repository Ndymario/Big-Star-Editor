-- Ported from NSMB Editor 5's NSMBStageObj.cs case 159.

local images = {}

local function init(ctx)
    images.MidpointFlag = ctx.loadImage("images/MidpointFlag.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 22
    height = 33
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.MidpointFlag, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 159,
    name = "Checkpoint",
    category = "Other",
    notes = "The midway point. The ID of the Start position needs to be configured in the View's settings.",
    fields = {
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
