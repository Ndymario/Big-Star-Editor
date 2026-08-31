-- Ported from NSMB Editor 5's NSMBStageObj.cs case 99.

local images = {}

local function init(ctx)
    images.PiranhaplantGround = ctx.loadImage("images/PiranhaplantGround.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y + 3
    width = 33
    height = 29
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.PiranhaplantGround, RenderX, RenderY + 3) or drawn
    return drawn
end

return {
    objectId = 99,
    name = "Grounded Piranha Plant",
    category = "Enemies - Overground",
    notes = "Unused behaviors:\n- Can be defeated by Drill Stomp",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
