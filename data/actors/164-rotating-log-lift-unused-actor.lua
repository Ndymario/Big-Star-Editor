-- Ported from NSMB Editor 5's NSMBStageObj.cs case 164.

local images = {}

local function init(ctx)
    images.SpinningLog = ctx.loadImage("images/SpinningLog.png")
    images.SpinningLogL = ctx.loadImage("images/SpinningLogL.png")
end

local function grabBounds(actor)
    if actor.fields.longer then
        return { x = actor.pixelX - 56, y = actor.pixelY, width = 128, height = 16 }
    end
    return { x = actor.pixelX - 24, y = actor.pixelY, width = 64, height = 16 }
end

local function render(actor, renderer)
    local image = actor.fields.longer and images.SpinningLogL or images.SpinningLog
    return renderer:draw(image, grabBounds(actor))
end

return {
    objectId = 164,
    name = "Rotating log lift (unused actor)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "longer",
            label = "Longer",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
