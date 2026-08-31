-- Ported from NSMB Editor 5's NSMBStageObj.cs case 57.

local images = {}

local function init(ctx)
    images.FireBall = ctx.loadImage("images/FireBall.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    height = 29
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.FireBall, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 57,
    name = "Lava Bubble (Podoboo)",
    category = "Enemies - Fire",
    notes = "Use to be known as Podoboo, back in the days of Super Mario Bros.\nGoes back and forth from sprite position and lava/bottom of the view.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
