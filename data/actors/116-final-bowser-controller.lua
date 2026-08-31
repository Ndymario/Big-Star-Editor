-- Ported from NSMB Editor 5's NSMBStageObj.cs case 116.

local images = {}

local function init(ctx)
    images.FinalBoss = ctx.loadImage("images/FinalBoss.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 125
    y = y - 28
    width = 162
    height = 43
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.FinalBoss, RenderX - 125, RenderY - 28) or drawn
    return drawn
end

return {
    objectId = 116,
    name = "Final Bowser controller",
    category = "Bosses and cutscenes",
    notes = "Spawns Bowser Jr. and Big Bowser. See those sprites for more info. Also makes peach say \"Help!\" When on screen.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
