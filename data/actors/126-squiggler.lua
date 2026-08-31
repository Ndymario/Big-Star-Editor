-- Ported from NSMB Editor 5's NSMBStageObj.cs case 126.

local images = {}

local function init(ctx)
    images.PipeCaterpiller = ctx.loadImage("images/PipeCaterpiller.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.pipe_exit_facing == 1 then
        y = y + 16
    else
        y = y - 15
    end
    x = x + 10
    width = 13
    height = 15
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.PipeCaterpiller
    if actor.fields.pipe_exit_facing == 1 then
        img = gfx.transform(img, 180, false, false)
        drawn = gfx.blit(renderer, img, RenderX + 10, RenderY + 16) or drawn
    else
        drawn = gfx.blit(renderer, img, RenderX + 10, RenderY - 15) or drawn
    end
    return drawn
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
