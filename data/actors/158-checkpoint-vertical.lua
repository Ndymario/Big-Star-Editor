-- Ported from NSMB Editor 5's NSMBStageObj.cs case 158.

local images = {}

local function init(ctx)
    images.MidpointFlag = ctx.loadImage("images/MidpointFlag.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 22, height = 33 }
end

local function render(actor, renderer)
    return renderer:draw(images.MidpointFlag, grabBounds(actor))
end

return {
    objectId = 158,
    name = "Checkpoint - vertical",
    category = "Other",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
