-- Ported from NSMB Editor 5's NSMBStageObj.cs case 128.

local images = {}

local function init(ctx)
    images.MummiPokey = ctx.loadImage("images/MummiPokey.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 256
    y = y + 16
    width = 512
    height = 95
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.MummiPokey, RenderX - 256, RenderY + 16) or drawn
    return drawn
end

return {
    objectId = 128,
    name = "Mummipokey",
    category = "Bosses and cutscenes",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
