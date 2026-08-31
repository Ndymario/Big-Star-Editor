-- Ported from NSMB Editor 5's NSMBStageObj.cs case 266.

local images = {}

local function init(ctx)
    images.BossKey = ctx.loadImage("images/BossKey.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 20, y = actor.pixelY - 15, width = 25, height = 31 }
end

local function render(actor, renderer)
    return renderer:draw(images.BossKey, { x = actor.pixelX - 20, y = actor.pixelY - 15 })
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
