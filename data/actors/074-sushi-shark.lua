-- Ported from NSMB Editor 5's NSMBStageObj.cs case 74.

local images = {}

local function init(ctx)
    images.Shark = ctx.loadImage("images/Shark.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 27, y = actor.pixelY - 8, width = 71, height = 26 }
end

local function render(actor, renderer)
    return renderer:draw(images.Shark, grabBounds(actor))
end

return {
    objectId = 74,
    name = "Sushi (shark)",
    category = "Enemies - Water",
    notes = "Spawned by the Sushi spawner.\n\nThis sprite's settings seem to be an absolute mess of contradicting behaviours. ",
    fields = {
        {
            key = "unknown_5",
            label = "Unknown 5",
            bitOffset = 24,
            bitWidth = 4,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 12,
            bitWidth = 4,
            notes = "3 is unused. Requires 11 to be set to 1, 2 or 3.",
        },
        {
            key = "small",
            label = "Small",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "unknown_10",
            label = "Unknown 10",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Used as 1 by Sushi spawner.",
        },
        {
            key = "movement",
            label = "Movement",
            bitOffset = 0,
            bitWidth = 4,
            notes = "0 is unused. 2 used by Sushi spawner. Used as 3 by the sprite in 1-A.",
            choices = {
                { value = 0, label = "Back & forth" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
