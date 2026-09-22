-- Ported from NSMB Editor 5's NSMBStageObj.cs case 50.

local images = {}

local function init(ctx)
    images.TubeBubbles = ctx.loadImage("images/TubeBubbles.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 90, width = 32, height = 74 }
end

local function render(actor, renderer)
    return renderer:draw(images.TubeBubbles, grabBounds(actor))
end

return {
    objectId = 50,
    name = "Aquatic bubble current (up)",
    category = "Enemies - Water",
    notes = "This sprite is prone to self-duplication.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
