-- Ported from NSMB Editor 5's NSMBStageObj.cs case 102.

local images = {}

local function init(ctx)
    images.GiantFirePiranhaplant = ctx.loadImage("images/GiantFirePiranhaplant.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 38
    y = y - 45
    width = 79
    height = 61
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.GiantFirePiranhaplant, RenderX - 38, RenderY - 45) or drawn
    return drawn
end

return {
    objectId = 102,
    name = "Large grounded Fire Piranha Plant (unused actor)",
    category = "Enemies - Overground",
    notes = "Unused. Used in NSMBWii.\n\nUnused behaviours:\n- Can be defeated by Drill Stomp",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
