-- Ported from NSMB Editor 5's NSMBStageObj.cs case 175.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.Spinning3PointedLargePlatform = ctx.loadImage("images/Spinning3PointedLargePlatform.png")
    images.Spinning3PointedPlatform = ctx.loadImage("images/Spinning3PointedPlatform.png")
end

local function grabBounds(actor)
    if actor.fields.longer_steps then
        return { x = actor.pixelX - 41, y = actor.pixelY - 41, width = 121, height = 99 }
    end
    return { x = actor.pixelX - 28, y = actor.pixelY - 34, width = 92, height = 85 }
end

local function render(actor, renderer)
    local platform = actor.fields.longer_steps
        and images.Spinning3PointedLargePlatform
        or images.Spinning3PointedPlatform
    renderer:draw(platform, grabBounds(actor))
    return renderer:draw(images.Clockwise, {
        x = actor.pixelX - 7,
        y = actor.pixelY - 8,
        width = 32,
        height = 32,
        flipX = actor.fields.rotation == 0,
    })
end

return {
    objectId = 175,
    name = "Three platform rickshaw lift (rotates in place by self)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 16,
            bitWidth = 1,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "slower",
            label = "Slower",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "longer_steps",
            label = "Longer steps",
            bitOffset = 0,
            bitWidth = 1,
            notes = "Unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
