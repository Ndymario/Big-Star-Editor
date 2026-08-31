-- Ported from NSMB Editor 5's NSMBStageObj.cs case 101.

local images = {}

local function init(ctx)
    images.FirePiranhaplant = ctx.loadImage("images/FirePiranhaplant.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 8, y = actor.pixelY + 2, width = 40, height = 30 }
end

local function render(actor, renderer)
    return renderer:draw(images.FirePiranhaplant, grabBounds(actor))
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
