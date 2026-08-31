-- Ported from NSMB Editor 5's NSMBStageObj.cs case 70.

local images = {}

local function init(ctx)
    images.GiantHammerBro = ctx.loadImage("images/GiantHammerBro.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 15
    width = 26
    height = 31
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.GiantHammerBro, RenderX, RenderY - 15) or drawn
    return drawn
end

return {
    objectId = 70,
    name = "Sledge Bro.",
    category = "Enemies - Overground",
    notes = "Can destroy smashable (question mark/brick/wood/stone/used) blocks.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
