-- Ported from NSMB Editor 5's NSMBStageObj.cs case 117.

local images = {}

local function init(ctx)
    images.HangingScuttleBug = ctx.loadImage("images/HangingScuttleBug.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y + ((d[5] & 0xF0) + 64)
    x = x - 14
    width = 44
    height = 37
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderY2 = RenderY
    local drawn = false
    RenderY = RenderY + ((d[5] & 0xF0) + 64)
    drawn = renderer:drawLine({ x = RenderX + 7, y = RenderY2, x2 = RenderX + 7, y2 = RenderY, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = RenderX + 8, y = RenderY2, x2 = RenderX + 8, y2 = RenderY, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = gfx.blit(renderer, images.HangingScuttleBug, RenderX - 14, RenderY) or drawn
    return drawn
end

return {
    objectId = 117,
    name = "Scuttlebug",
    category = "Enemies - Overground",
    notes = "A spider that will either drop to the ground or hang on it's web.",
    fields = {
        {
            key = "don_t_wait_for_player",
            label = "Don't wait for player",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Doesn't wait for player to get near to drop/move down. Used by Scuttlebug Spawner.",
        },
        {
            key = "drop_height",
            label = "Drop height",
            bitOffset = 4,
            bitWidth = 4,
            valueOffset = 4,
        },
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1=ignores nybble 10. 2=same as 1 but it falls off screen when stomped instead of getting squished; unused.",
            choices = {
                { value = 0, label = "Hangs on web" },
                { value = 1, label = "Drops on ground" },
                { value = 2, label = "Drops on ground (different stomp)" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
