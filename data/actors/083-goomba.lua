-- Ported from NSMB Editor 5's NSMBStageObj.cs case 83.

local images = {}

local function init(ctx)
    images.Goomba = ctx.loadImage("images/Goomba.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    y = y - 2
    height = 18
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Goomba, RenderX, RenderY - 2) or drawn
    return drawn
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
