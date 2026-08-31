-- Ported from NSMB Editor 5's NSMBStageObj.cs case 100.

local images = {}

local function init(ctx)
    images.GiantPiranhaplant = ctx.loadImage("images/GiantPiranhaplant.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 37, y = actor.pixelY - 48, width = 64, height = 64 }
end

local function render(actor, renderer)
    return renderer:draw(images.GiantPiranhaplant, grabBounds(actor))
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
