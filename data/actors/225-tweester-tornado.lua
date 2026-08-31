-- Ported from NSMB Editor 5's NSMBStageObj.cs case 225.

local images = {}

local function init(ctx)
    images.Tornado = ctx.loadImage("images/Tornado.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 88, y = actor.pixelY - 130, width = 191, height = 150 }
end

local function render(actor, renderer)
    return renderer:draw(images.Tornado, grabBounds(actor))
end

return {
    objectId = 225,
    name = "Tweester (Tornado)",
    category = "Enemies - Desert",
    notes = "This tornado, may or may not be the same thing as a Tweester.\n\nSlings you up in the air, after which you can Drill Stomp.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
