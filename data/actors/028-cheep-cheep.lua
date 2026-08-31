-- Ported from NSMB Editor 5's NSMBStageObj.cs case 28.

local gfx = {}
local function init(ctx)
    gfx.plain = ctx.loadImage("images/CheepCheep.png")
    gfx.green = ctx.loadImage("images/CheepCheepGreen.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 1, y = actor.pixelY + 4, width = 19, height = 18 }
end

local function render(actor, renderer)
    local image = actor.fields.type == 1 and gfx.green or gfx.plain
    return renderer:draw(image, { x = actor.pixelX - 1, y = actor.pixelY + 4, width = 19, height = 18 })
end

return {
    objectId = 28,
    name = "Cheep Cheep",
    category = "Enemies - Water",
    notes = "There are more values that are used by the Cheep Cheep spawner.",
    fields = {
        {
            key = "slower",
            label = "Slower",
            bitOffset = 12,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "type",
            label = "Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "'Swims' on walls: makes fish follow the ground and swim up walls and on ceilings.",
            choices = {
                { value = 0, label = "Swim" },
                { value = 1, label = "Green (chases Mario)" },
                { value = 2, label = "Jumping (unused)" },
                { value = 3, label = "'Swims' on walls (unused)" },
            },
        },
        {
            key = "wall_placement",
            label = "Wall placement",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Only works with 'swims on walls' type.",
            choices = {
                { value = 0, label = "Top" },
                { value = 1, label = "Left" },
                { value = 2, label = "Below" },
                { value = 3, label = "Right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
