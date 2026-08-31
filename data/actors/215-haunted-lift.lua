-- Ported from NSMB Editor 5's NSMBStageObj.cs case 215.

local images = {}

local function init(ctx)
    images.GhostPlatform = ctx.loadImage("images/GhostPlatform.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y + (-8 + actor.fields.y_shift_pixels)
    x = x - 64
    width = 128
    height = 16
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    RenderY = RenderY + (-8 + actor.fields.y_shift_pixels)
    drawn = gfx.blit(renderer, images.GhostPlatform, RenderX - 64, RenderY) or drawn
    return drawn
end

return {
    objectId = 215,
    name = "Haunted lift",
    category = "Enemies - Ghost House",
    fields = {
        {
            key = "y_shift_pixels",
            label = "Y-Shift (pixels)",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
