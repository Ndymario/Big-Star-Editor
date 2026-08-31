-- Ported from NSMB Editor 5's NSMBStageObj.cs case 116.

local images = {}

local function init(ctx)
    images.FinalBoss = ctx.loadImage("images/FinalBoss.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 125, y = actor.pixelY - 28, width = 162, height = 43 }
end

local function render(actor, renderer)
    return renderer:draw(images.FinalBoss, { x = actor.pixelX - 125, y = actor.pixelY - 28 })
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
