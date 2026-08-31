-- Ported from NSMB Editor 5's NSMBStageObj.cs case 232.

local images = {}

local function init(ctx)
    images.TightRopeEnd = ctx.loadImage("images/TightRopeEnd.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 8
    width = (actor.fields.length + 1) * 16
    height = (d[2] % 0x10)
    if height >= 8 then
        height = height - 16
    end
    if height >= 0 then
        y = y - (height * 16)
        height = height + 1
    else
        height = height - 1
    end
    height = height * 16
    height = math.abs(height)
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local drawn = false
    RenderX = RenderX - 8
    RenderX2 = RenderX + actor.fields.length * 16
    RenderY2 = d[2] % 0x10
    if RenderY2 >= 8 then
        RenderY2 = RenderY2 - 16
    end
    RenderY2 = RenderY - RenderY2 * 16
    drawn = renderer:drawLine({ x = RenderX + 8, y = RenderY + 8, x2 = RenderX2 + 8, y2 = RenderY2 + 8, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = gfx.blit(renderer, images.TightRopeEnd, RenderX, RenderY) or drawn
    drawn = gfx.blit(renderer, images.TightRopeEnd, RenderX2, RenderY2) or drawn
    return drawn
end

return {
    objectId = 232,
    name = "Tightrope",
    category = "Platforms and ropes",
    notes = "Usable underwater, but may glitch if the rope part of the sprite is allowed to touch a solid tile while Mario/Luigi walks or stands on it. ",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 28,
            bitWidth = 4,
            notes = "If both length and height are set, it will be diagonal.",
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 24,
            bitWidth = 4,
            signed = true,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
