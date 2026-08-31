-- Ported from NSMB Editor 5's NSMBStageObj.cs case 199.

local images = {}

local function init(ctx)
    images.GroundpoundGoo = ctx.loadImage("images/GroundpoundGoo.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = (d[5] % 0x10 + 1) * 16
    height = (d[5] // 0x10 + 1) * 16
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.GroundpoundGoo, RenderX, RenderY, (d[5] % 0x10 + 1) * 16, (d[5] // 0x10 + 1) * 16) or drawn
    return drawn
end

return {
    objectId = 199,
    name = "Groundpound-able ghost house goo",
    category = "Platforms and ropes",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            valueOffset = 1,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 4,
            bitWidth = 4,
            valueOffset = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
