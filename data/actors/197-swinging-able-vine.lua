-- Ported from NSMB Editor 5's NSMBStageObj.cs case 197.

local images = {}

local function init(ctx)
    images.Vine = ctx.loadImage("images/Vine.png")
    images.VineBottom = ctx.loadImage("images/VineBottom.png")
    images.VineTop = ctx.loadImage("images/VineTop.png")
end

local function grabBounds(actor)
    local extraSegments = math.min(actor.fields.length, 4)
    return {
        x = actor.pixelX - 16,
        y = actor.pixelY,
        width = 32,
        height = 16 * (extraSegments + 4),
    }
end

local function render(actor, renderer)
    local x = actor.pixelX - 16
    local middleSegments = math.min(actor.fields.length, 4) + 2
    renderer:draw(images.VineTop, { x = x, y = actor.pixelY })
    for segment = 1, middleSegments do
        renderer:draw(images.Vine, { x = x, y = actor.pixelY + 16 * segment })
    end
    return renderer:draw(images.VineBottom, {
        x = x,
        y = actor.pixelY + 16 * (middleSegments + 1),
    })
end

return {
    objectId = 197,
    name = "Swinging/able vine",
    category = "Platforms and ropes",
    notes = "Requires BOTH sprite sets \"Vine\" and \"Rope\".\nIf not, it crashes.",
    fields = {
        {
            key = "swing_angle",
            label = "Swing angle",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "dont_restrict_angle",
            label = "Dont restrict angle",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
