-- Ported from NSMB Editor 5's NSMBStageObj.cs case 91.

local images = {}

local function init(ctx)
    images.Snailicorn = ctx.loadImage("images/Snailicorn.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 16, width = 30, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.Snailicorn, grabBounds(actor))
end

return {
    objectId = 91,
    name = "Snailicorn",
    category = "Enemies - Snow",
    notes = "Pushes the player, does collision damage from the front. To kill it, you can either push it off a cliff, use a star, or use a mega mushroom.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
