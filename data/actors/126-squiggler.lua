-- Ported from NSMB Editor 5's NSMBStageObj.cs case 126.

local images = {}

local function init(ctx)
    images.PipeCaterpiller = ctx.loadImage("images/PipeCaterpiller.png")
end

local function grabBounds(actor)
    if actor.fields.pipe_exit_facing == 1 then
        return { x = actor.pixelX + 10, y = actor.pixelY + 16, width = 13, height = 15 }
    end
    return { x = actor.pixelX + 10, y = actor.pixelY - 15, width = 13, height = 15 }
end

local function render(actor, renderer)
    local bounds = grabBounds(actor)
    if actor.fields.pipe_exit_facing == 1 then
        bounds.rotation = 180
    end
    return renderer:draw(images.PipeCaterpiller, bounds)
end

return {
    objectId = 126,
    name = "Squiggler",
    category = "Enemies - Underground",
    notes = "A caterpillar that is smaller than the Wiggler. Crawls slowly on all sides of  a surface.",
    fields = {
        {
            key = "pipe_exit_facing",
            label = "Pipe exit facing",
            bitOffset = 0,
            bitWidth = 4,
            notes = "2 and 3 are unused.",
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 3, label = "Left" },
                { value = 2, label = "Right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
