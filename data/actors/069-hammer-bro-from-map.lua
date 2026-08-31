-- Ported from NSMB Editor 5's NSMBStageObj.cs case 69.

local images = {}

local function init(ctx)
    images.HammerBro = ctx.loadImage("images/HammerBro.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 18, width = 18, height = 34 }
end

local function render(actor, renderer)
    return renderer:draw(images.HammerBro, grabBounds(actor))
end

return {
    objectId = 69,
    name = "Hammer Bro. from map",
    category = "Enemies - Overground",
    notes = "If a Hammer Bro is on the level this actor is placed in, upon starting the level a Hammer Bro. will spawn there.",
    fields = {
        {
            key = "loads_from",
            label = "Loads from",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Level Start" },
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
            key = "disable_downward_jump",
            label = "Disable downward jump",
            bitOffset = 8,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
