-- Ported from NSMB Editor 5's NSMBStageObj.cs case 102.

local images = {}

local function init(ctx)
    images.GiantFirePiranhaplant = ctx.loadImage("images/GiantFirePiranhaplant.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 38, y = actor.pixelY - 45, width = 79, height = 61 }
end

local function render(actor, renderer)
    return renderer:draw(images.GiantFirePiranhaplant, grabBounds(actor))
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
