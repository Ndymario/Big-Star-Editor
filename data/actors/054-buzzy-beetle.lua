-- Ported from NSMB Editor 5's NSMBStageObj.cs case 54.

local images = {}

local function init(ctx)
    images.BuzzyBeetle = ctx.loadImage("images/BuzzyBeetle.png")
    images.BuzzyBeetleU = ctx.loadImage("images/BuzzyBeetleU.png")
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    if actor.fields.spawn_as == 1 then
        img = images.BuzzyBeetleU
    else
        img = images.BuzzyBeetle
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
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
