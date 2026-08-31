-- Ported from NSMB Editor 5's NSMBStageObj.cs case 83.

local images = {}

local function init(ctx)
    images.Goomba = ctx.loadImage("images/Goomba.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 2, width = 16, height = 18 }
end

local function render(actor, renderer)
    return renderer:draw(images.Goomba, grabBounds(actor))
end

return {
    objectId = 83,
    name = "Goomba",
    category = "Enemies - Overground",
    notes = "This is the classic enemy that we can find in all Super Mario games.",
    fields = {
        {
            key = "spawn_type",
            label = "Spawn Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Makes the Goomba spawn differently. 1 used for Mega Mario ground pound. 2 used for Enemy Spawner.",
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Spinning" },
                { value = 2, label = "Rising" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
