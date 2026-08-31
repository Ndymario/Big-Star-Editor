-- Ported from NSMB Editor 5's NSMBStageObj.cs case 234.

local images = {}

local function init(ctx)
    images.Flamethrower = ctx.loadImage("images/Flamethrower.png")
    images.SmallFlamethrower = ctx.loadImage("images/SmallFlamethrower.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    local w = 64
    if actor.objectId == 235 then
        w = 128
    end
    if d[5] % 0x40 == 0 then
        width = w
        height = 16
    elseif d[5] % 0x40 == 1 then
        x = x - (w - 16)
        width = w
        height = 16
    elseif d[5] % 0x40 == 2 then
        y = y - (w - 16)
        width = 16
        height = w
    elseif d[5] % 0x40 == 3 then
        width = 16
        height = w
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    if actor.objectId == 234 then
        img = images.SmallFlamethrower
    else
        img = images.Flamethrower
    end
    if d[5] % 0x40 == 0 then
        drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    elseif d[5] % 0x40 == 1 then
        img = gfx.transform(img, 180, false, false)
        drawn = gfx.blit(renderer, img, RenderX - (gfx.width(img) - 16), RenderY) or drawn
    elseif d[5] % 0x40 == 2 then
        img = gfx.transform(img, 270, false, false)
        drawn = gfx.blit(renderer, img, RenderX, RenderY - (gfx.height(img) - 16)) or drawn
    elseif d[5] % 0x40 == 3 then
        img = gfx.transform(img, 90, false, false)
        drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    end
    return drawn
end

return {
    objectId = 234,
    name = "Burner (Small)",
    category = "Enemies - Castle",
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
                { value = 2, label = "Up" },
                { value = 3, label = "Down" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
