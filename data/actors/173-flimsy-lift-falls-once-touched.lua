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
            bitWidth = 2,
            notes = "The platform is twice this many tiles wide. Everything other than 1 is unused; some lengths shift it upwards in-game. The game reads two bits, so the range is 1 to 4.",
            valueOffset = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
