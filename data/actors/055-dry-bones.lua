-- Ported from NSMB Editor 5's NSMBStageObj.cs case 55.

local images = {}

local function init(ctx)
    images.DryBones = ctx.loadImage("images/DryBones.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 10
    width = 17
    height = 26
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.DryBones, RenderX, RenderY - 10) or drawn
    return drawn
end

return {
    objectId = 55,
    name = "Dry Bones",
    category = "Enemies - Castle",
    notes = "Ignores water.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
