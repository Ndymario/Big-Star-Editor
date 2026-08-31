-- Ported from NSMB Editor 5's NSMBStageObj.cs case 173.

local images = {}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
end

local function grabBounds(actor)
    local tiles = actor.fields.length * 2
    return {
        x = actor.pixelX - tiles * 8 + 8,
        y = actor.pixelY,
        width = tiles * 16,
        height = 16,
    }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    local tiles = actor.fields.length * 2
    renderer:draw(images.MovingPlatformLeft, { x = box.x, y = box.y })
    for column = 1, tiles - 2 do
        renderer:draw(images.MovingPlatformSection, { x = box.x + 16 * column, y = box.y })
    end
    return renderer:draw(images.MovingPlatformRight, { x = box.x + 16 * (tiles - 1), y = box.y })
end

return {
    objectId = 173,
    name = "Flimsy Lift (falls once touched)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 3,
            notes = "The platform is twice this many tiles wide. Everything other than 1 is unused; some lengths shift it upwards in-game.",
            valueOffset = 1,
        },
        {
            key = "length_2",
            label = "length",
            bitOffset = 40,
            bitWidth = 4,
            notes = "do not exceed 4 or it will not be displayed",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
