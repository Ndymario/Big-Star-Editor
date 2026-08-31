-- Ported from NSMB Editor 5's NSMBStageObj.cs case 225.

local images = {}

local function init(ctx)
    images.Tornado = ctx.loadImage("images/Tornado.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 88
    y = y - 130
    width = 191
    height = 150
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Tornado, RenderX - 88, RenderY - 130) or drawn
    return drawn
end

return {
    objectId = 225,
    name = "Tweester (Tornado)",
    category = "Enemies - Desert",
    notes = "This tornado, may or may not be the same thing as a Tweester.\n\nSlings you up in the air, after which you can Drill Stomp.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
