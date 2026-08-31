-- Ported from NSMB Editor 5's NSMBStageObj.cs case 117.

local images = {}

local function init(ctx)
    images.HangingScuttleBug = ctx.loadImage("images/HangingScuttleBug.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 14,
        y = actor.pixelY + actor.fields.drop_height * 16,
        width = 44,
        height = 37,
    }
end

local function render(actor, renderer)
    local bounds = grabBounds(actor)
    local drawn = false
    drawn = renderer:drawLine({ x = actor.pixelX + 7, y = actor.pixelY, x2 = actor.pixelX + 7, y2 = bounds.y, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = actor.pixelX + 8, y = actor.pixelY, x2 = actor.pixelX + 8, y2 = bounds.y, color = "#ffffff", strokeWidth = 1 }) or drawn
    return renderer:draw(images.HangingScuttleBug, bounds) or drawn
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
