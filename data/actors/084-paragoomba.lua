-- Ported from NSMB Editor 5's NSMBStageObj.cs case 84.

local images = {}

local function init(ctx)
    images.Paragoomba = ctx.loadImage("images/Paragoomba.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 12, width = 24, height = 28 }
end

local function render(actor, renderer)
    return renderer:draw(images.Paragoomba, grabBounds(actor))
end

return {
    objectId = 84,
    name = "Paragoomba",
    category = "Enemies - Overground",
    notes = "Unused behaviours:\n- Has ceiling detection [may be used]\n- Has water interaction [may be used, or Goomba leftover]",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
