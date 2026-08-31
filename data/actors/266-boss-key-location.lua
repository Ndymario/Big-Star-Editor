-- Ported from NSMB Editor 5's NSMBStageObj.cs case 266.

local images = {}

local function init(ctx)
    images.BossKey = ctx.loadImage("images/BossKey.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 20
    y = y - 15
    width = 25
    height = 31
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.BossKey, RenderX - 20, RenderY - 15) or drawn
    return drawn
end

return {
    objectId = 266,
    name = "Boss Key Location",
    category = "Bosses and cutscenes",
    notes = "If the boss is defeated, the Boss Battle Key falls on the place where you placed this sprite.\nIf else it falls in a default place?",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
