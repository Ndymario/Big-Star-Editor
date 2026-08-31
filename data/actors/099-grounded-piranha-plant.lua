-- Ported from NSMB Editor 5's NSMBStageObj.cs case 99.

local images = {}

local function init(ctx)
    images.PiranhaplantGround = ctx.loadImage("images/PiranhaplantGround.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY + 3, width = 33, height = 29 }
end

local function render(actor, renderer)
    return renderer:draw(images.PiranhaplantGround, grabBounds(actor))
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
