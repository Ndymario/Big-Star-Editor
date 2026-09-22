-- Ported from NSMB Editor 5's NSMBStageObj.cs case 119.

local images = {}

local function init(ctx)
    images.PeteyPiranha = ctx.loadImage("images/PeteyPiranha.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 16, y = actor.pixelY - 56, width = 108, height = 75 }
end

local function render(actor, renderer)
    return renderer:draw(images.PeteyPiranha, grabBounds(actor))
end

return {
    objectId = 119,
    name = "Petey Piranha",
    category = "Bosses and cutscenes",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
