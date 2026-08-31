-- Ported from NSMB Editor 5's NSMBStageObj.cs case 55.

local images = {}

local function init(ctx)
    images.DryBones = ctx.loadImage("images/DryBones.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 10, width = 17, height = 26 }
end

local function render(actor, renderer)
    return renderer:draw(images.DryBones, grabBounds(actor))
end

return {
    objectId = 55,
    name = "Dry Bones",
    category = "Enemies - Castle",
    notes = "Ignores water.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
