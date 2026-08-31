-- Ported from NSMB Editor 5's NSMBStageObj.cs case 100.

local images = {}

local function init(ctx)
    images.GiantPiranhaplant = ctx.loadImage("images/GiantPiranhaplant.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 37
    y = y - 48
    width = 64
    height = 64
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.GiantPiranhaplant, RenderX - 37, RenderY - 48) or drawn
    return drawn
end

return {
    objectId = 100,
    name = "Big Grounded Piranha Plant",
    category = "Enemies - Overground",
    notes = "Unused behaviours:\n- Can be defeated by Drill Stomp",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
