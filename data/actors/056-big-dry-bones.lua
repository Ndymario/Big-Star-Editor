-- Ported from NSMB Editor 5's NSMBStageObj.cs case 56.

local images = {}

local function init(ctx)
    images.DryBonesLarge = ctx.loadImage("images/DryBonesLarge.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 28
    width = 25
    height = 44
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.DryBonesLarge, RenderX, RenderY - 28) or drawn
    return drawn
end

return {
    objectId = 56,
    name = "Big Dry Bones",
    category = "Enemies - Castle",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
