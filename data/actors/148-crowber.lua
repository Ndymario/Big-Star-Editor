-- Ported from NSMB Editor 5's NSMBStageObj.cs case 148.

local images = {}

local function init(ctx)
    images.Crow = ctx.loadImage("images/Crow.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 22, height = 14 }
end

local function render(actor, renderer)
    return renderer:draw(images.Crow, grabBounds(actor))
end

return {
    objectId = 148,
    name = "Crowber",
    category = "Enemies - Ghost House",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
