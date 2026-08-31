-- Ported from NSMB Editor 5's NSMBStageObj.cs case 200.

local images = {}

local function init(ctx)
    images.SinkingSnow = ctx.loadImage("images/SinkingSnow.png")
    images.SinkingSnowEdge = ctx.loadImage("images/SinkingSnowEdge.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 16,
        y = actor.pixelY,
        width = 16 * (actor.fields.length + 3),
        height = 35,
    }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    renderer:draw(images.SinkingSnowEdge, { x = box.x, y = box.y })
    for section = 0, actor.fields.length do
        renderer:draw(images.SinkingSnow, { x = box.x + 16 * (section + 1), y = box.y })
    end
    return renderer:draw(images.SinkingSnowEdge, {
        x = box.x + 16 * (actor.fields.length + 2),
        y = box.y,
        flipX = true,
    })
end

return {
    objectId = 200,
    name = "Sinking-snow pile",
    category = "Enemies - Snow",
    notes = "Length of 0 makes the game crash!",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Game will crash if set to 0!",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
