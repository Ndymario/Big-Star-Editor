-- Ported from NSMB Editor 5's NSMBStageObj.cs case 135.

local images = {}

local function init(ctx)
    images.CheepSkipper = ctx.loadImage("images/CheepSkipper.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX + 22, y = actor.pixelY - 29, width = 59, height = 48 }
end

local function render(actor, renderer)
    return renderer:draw(images.CheepSkipper, grabBounds(actor))
end

return {
    objectId = 135,
    name = "Cheepskipper",
    category = "Bosses and cutscenes",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
