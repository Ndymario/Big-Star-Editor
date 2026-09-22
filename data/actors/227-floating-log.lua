-- Ported from NSMB Editor 5's NSMBStageObj.cs case 227.

local images = {}

local function init(ctx)
    images.Log = ctx.loadImage("images/Log.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 128, y = actor.pixelY - 32, width = 256, height = 64 }
end

local function render(actor, renderer)
    return renderer:draw(images.Log, grabBounds(actor))
end

return {
    objectId = 227,
    name = "Floating log",
    category = "Platforms and ropes",
    notes = "Floating log reads no sprite data: neither the settings word nor either event byte changes what it does. Levels set the fifth nibble to 5 regardless.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
