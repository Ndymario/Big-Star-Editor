-- Ported from NSMB Editor 5's NSMBStageObj.cs case 57.

local images = {}

local function init(ctx)
    images.FireBall = ctx.loadImage("images/FireBall.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 29 }
end

local function render(actor, renderer)
    return renderer:draw(images.FireBall, grabBounds(actor))
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
