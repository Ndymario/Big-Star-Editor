-- Ported from NSMB Editor 5's NSMBStageObj.cs case 187.

local images = {}

local function init(ctx)
    images.StarGate = ctx.loadImage("images/StarGate.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 32, height = 8 }
end

local function render(actor, renderer)
    return renderer:draw(images.StarGate, grabBounds(actor))
end

return {
    objectId = 187,
    name = "Ground-pound panel",
    category = "Platforms and ropes",
    notes = "Needs a ground-pound to be able to pass from top. Is jump-through from bottom.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
