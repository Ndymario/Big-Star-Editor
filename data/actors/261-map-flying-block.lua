-- Ported from NSMB Editor 5's NSMBStageObj.cs case 261.

local images = {}

local function init(ctx)
    images.FlyingBlock = ctx.loadImage("images/FlyingBlock.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 12
    y = y - 8
    width = 44
    height = 23
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.FlyingBlock, RenderX - 12, RenderY - 8) or drawn
    return drawn
end

return {
    objectId = 261,
    name = "Map Flying ? Block",
    category = "Other",
    notes = "Moves slowly if in water.",
    fields = {
        {
            key = "loads_from",
            label = "Loads from",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Level start" },
                { value = 1, label = "Midway point" },
            },
        },
        {
            key = "can_spawn_mega_mushroom",
            label = "Can spawn Mega Mushroom",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Left" },
                { value = 1, label = "Right" },
            },
        },
        {
            key = "looping_pattern",
            label = "Looping pattern",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Fly off screen" },
                { value = 1, label = "Center of Loop" },
                { value = 2, label = "Edge of Loop" },
            },
        },
        {
            key = "flying_pattern",
            label = "Flying pattern",
            bitOffset = 12,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Back and forth" },
                { value = 1, label = "Forward 4/back 8" },
                { value = 2, label = "Stays in place" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
