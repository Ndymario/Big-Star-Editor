-- Ported from NSMB Editor 5's NSMBStageObj.cs case 210.

local images = {}

local function init(ctx)
    images.ThruWallPlatform = ctx.loadImage("images/ThruWallPlatform.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 96
    height = 16
    if actor.fields.direction == 0 then
        x = x - 80
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.ThruWallPlatform
    if actor.fields.direction == 0 then
        drawn = gfx.blit(renderer, img, RenderX - 80, RenderY) or drawn
    else
        img = gfx.transform(img, 0, true, false)
        drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    end
    return drawn
end

return {
    objectId = 210,
    name = "Rotating carry-through-wall turntable-like lift",
    category = "Platforms and ropes",
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "From left to right" },
                { value = 1, label = "From right to left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
