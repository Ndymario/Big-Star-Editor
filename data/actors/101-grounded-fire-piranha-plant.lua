-- Ported from NSMB Editor 5's NSMBStageObj.cs case 101.

local images = {}

local function init(ctx)
    images.FirePiranhaplant = ctx.loadImage("images/FirePiranhaplant.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 8
    y = y + 2
    width = 40
    height = 30
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.FirePiranhaplant, RenderX - 8, RenderY + 2) or drawn
    return drawn
end

return {
    objectId = 101,
    name = "Grounded Fire Piranha Plant",
    category = "Enemies - Overground",
    notes = "Unused behaviours:\n- Can be defeated by Drill Stomp",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
