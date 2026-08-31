-- Ported from NSMB Editor 5's NSMBStageObj.cs case 56.

local images = {}

local function init(ctx)
    images.DryBonesLarge = ctx.loadImage("images/DryBonesLarge.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 28, width = 25, height = 44 }
end

local function render(actor, renderer)
    return renderer:draw(images.DryBonesLarge, grabBounds(actor))
end

return {
    objectId = 56,
    name = "Big Dry Bones",
    category = "Enemies - Castle",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
