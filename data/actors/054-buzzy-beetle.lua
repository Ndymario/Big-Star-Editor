-- Ported from NSMB Editor 5's NSMBStageObj.cs case 54.

local images = {}

local function init(ctx)
    images.BuzzyBeetle = ctx.loadImage("images/BuzzyBeetle.png")
    images.BuzzyBeetleU = ctx.loadImage("images/BuzzyBeetleU.png")
end

local function render(actor, renderer)
    local image = actor.fields.spawn_as == 1 and images.BuzzyBeetleU or images.BuzzyBeetle
    return renderer:draw(image, { x = actor.pixelX, y = actor.pixelY })
end

return {
    objectId = 54,
    name = "Buzzy Beetle",
    category = "Enemies - Underground",
    fields = {
        {
            key = "spawn_as",
            label = "Spawn as",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Default" },
                { value = 1, label = "Ceiling walker" },
                { value = 2, label = "Shell" },
                { value = 3, label = "Upside-down shell" },
            },
        },
    },
    init = init,
    render = render,
}
