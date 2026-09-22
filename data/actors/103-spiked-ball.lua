-- Ported from NSMB Editor 5's NSMBStageObj.cs case 103.

local images = {}

local function init(ctx)
    images.SpikeBallSmall = ctx.loadImage("images/SpikeBallSmall.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 30, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.SpikeBallSmall, grabBounds(actor))
end

return {
    objectId = 103,
    name = "Spiked Ball",
    category = "Enemies - Castle",
    notes = "Trigger event: the spiked ball won't spawn while that ID is activated; 0 disables it.\nTarget event: a second such ID, which has no effect if the trigger event is 0.",
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 4,
            bitWidth = 8,
            choices = {
                { value = 0, label = "Left" },
                { value = 1, label = "Right" },
                { value = 2, label = "Towards player" },
                { value = 16, label = "Towards player from right only" },
                { value = 17, label = "Towards player from left only" },
            },
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Slope only" },
                { value = 1, label = "60 pixels/sec" },
                { value = 2, label = "90 pixels/sec" },
                { value = 3, label = "120 pixels/sec" },
            },
        },
        {
            key = "bounce_on_walls_faster",
            label = "Bounce on walls faster",
            bitOffset = 12,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "spiked_ball_id",
            label = "Spiked Ball ID",
            bitOffset = 16,
            bitWidth = 4,
            notes = "For use with Spiked Ball Activator (46)",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
