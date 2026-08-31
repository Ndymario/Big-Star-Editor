-- Ported from NSMB Editor 5's NSMBStageObj.cs case 103.

local images = {}

local function init(ctx)
    images.SpikeBallSmall = ctx.loadImage("images/SpikeBallSmall.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 30
    height = 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.SpikeBallSmall, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 103,
    name = "Spiked Ball",
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
