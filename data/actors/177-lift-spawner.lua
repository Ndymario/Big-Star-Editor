-- Ported from NSMB Editor 5's NSMBStageObj.cs case 177.

local images = {}

local geometry = {
    [0] = { platformX = 0, platformY = 16, arrowX = 24, rotation = 180 },
    [1] = { platformX = 0, platformY = -16, arrowX = 24, rotation = 0 },
    [2] = { platformX = -64, platformY = 0, arrowX = 0, rotation = 270 },
    [3] = { platformX = 64, platformY = 0, arrowX = 48, rotation = 90 },
}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
    images.arrow_down = ctx.loadImage("images/arrow_down.png")
end

local function grabBounds(actor)
    local g = geometry[actor.fields.direction]
    return { x = actor.pixelX + g.platformX, y = actor.pixelY + g.platformY, width = 64, height = 16 }
end

local function render(actor, renderer)
    local g = geometry[actor.fields.direction]
    local box = grabBounds(actor)
    renderer:draw(images.MovingPlatformLeft, { x = box.x, y = box.y })
    renderer:draw(images.MovingPlatformSection, { x = box.x + 16, y = box.y })
    renderer:draw(images.MovingPlatformSection, { x = box.x + 32, y = box.y })
    renderer:draw(images.MovingPlatformRight, { x = box.x + 48, y = box.y })
    return renderer:draw(images.arrow_down, {
        x = actor.pixelX + g.arrowX,
        y = actor.pixelY,
        width = 16,
        height = 16,
        rotation = g.rotation,
    })
end

return {
    objectId = 177,
    name = "Lift spawner",
    category = "Platforms and ropes",
    fields = {
        {
            key = "offscreen_pos",
            label = "Offscreen pos.",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used in the MvsL level. This controls what the lift (spawner, surely?) does while off screen, best I can tell. Position is affected, but speed stays the same.",
            choices = {
                { value = 0, label = "Keep moving" },
                { value = 1, label = "Wait for Mario" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 16,
            bitWidth = 2,
            notes = "2 (Right) and 3 (Left) are unused.",
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 2, label = "Right" },
                { value = 3, label = "Left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
