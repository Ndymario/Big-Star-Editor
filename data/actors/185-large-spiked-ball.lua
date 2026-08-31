-- Ported from NSMB Editor 5's NSMBStageObj.cs case 185.

local images = {}

local function init(ctx)
    images.SpikeBallLarge = ctx.loadImage("images/SpikeBallLarge.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 42, width = 63, height = 58 }
end

local function render(actor, renderer)
    return renderer:draw(images.SpikeBallLarge, grabBounds(actor))
end

return {
    objectId = 185,
    name = "Large Spiked Ball",
    category = "Enemies - Castle",
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
            notes = "If set to true when it collides with a wall it will roll the other direction.",
            checkboxValue = 1,
        },
        {
            key = "destroyer_id_1",
            label = "Destroyer ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "The spiked ball won't spawn while the ID is activated. 0=disable",
        },
        {
            key = "destroyer_id_2",
            label = "Destroyer ID 2",
            bitOffset = 40,
            bitWidth = 8,
            notes = "This one will have no effect if Destroyer ID 1 is equal to 0.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
