-- Ported from NSMB Editor 5's NSMBStageObj.cs case 91.

local images = {}

local function init(ctx)
    images.Snailicorn = ctx.loadImage("images/Snailicorn.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 16
    width = 30
    height = 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Snailicorn, RenderX, RenderY - 16) or drawn
    return drawn
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
