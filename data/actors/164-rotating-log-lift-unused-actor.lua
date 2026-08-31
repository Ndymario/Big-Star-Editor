-- Ported from NSMB Editor 5's NSMBStageObj.cs case 164.

local images = {}

local function init(ctx)
    images.SpinningLog = ctx.loadImage("images/SpinningLog.png")
    images.SpinningLogL = ctx.loadImage("images/SpinningLogL.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width = 16
    if actor.fields.longer then
        x = x - 56
        width = 128
    else
        x = x - 24
        width = 64
    end
    return { x = x, y = y, width = width, height = 16 }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    if actor.fields.longer then
        drawn = gfx.blit(renderer, images.SpinningLogL, RenderX - 56, RenderY) or drawn
    else
        drawn = gfx.blit(renderer, images.SpinningLog, RenderX - 24, RenderY) or drawn
    end
    return drawn
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
