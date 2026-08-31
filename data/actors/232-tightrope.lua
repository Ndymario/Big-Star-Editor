-- Ported from NSMB Editor 5's NSMBStageObj.cs case 232.

local images = {}

local function init(ctx)
    images.TightRopeEnd = ctx.loadImage("images/TightRopeEnd.png")
end

local function grabBounds(actor)
    local endY = actor.pixelY - 16 * actor.fields.height
    return {
        x = actor.pixelX - 8,
        y = math.min(actor.pixelY, endY),
        width = 16 * (actor.fields.length + 1),
        height = math.abs(endY - actor.pixelY) + 16,
    }
end

local function render(actor, renderer)
    local startX = actor.pixelX - 8
    local endX = startX + 16 * actor.fields.length
    local endY = actor.pixelY - 16 * actor.fields.height
    renderer:drawLine({
        x = startX + 8,
        y = actor.pixelY + 8,
        x2 = endX + 8,
        y2 = endY + 8,
        color = "#ffffff",
        strokeWidth = 1,
    })
    renderer:draw(images.TightRopeEnd, { x = startX, y = actor.pixelY })
    return renderer:draw(images.TightRopeEnd, { x = endX, y = endY })
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
