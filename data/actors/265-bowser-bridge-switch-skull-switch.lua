-- Ported from NSMB Editor 5's NSMBStageObj.cs case 265.

local images = {}

local function init(ctx)
    images.BowserSwitch = ctx.loadImage("images/BowserSwitch.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x + 4
    y = y - 7
    width = 24
    height = 23
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.BowserSwitch, RenderX + 4, RenderY - 7) or drawn
    return drawn
end

return {
    objectId = 265,
    name = "Bowser bridge switch (skull switch)",
    category = "Event controllers",
    notes = "Destroys the bridge seen in the Boss area of 1-Castle, 8-First Castle, and 8-Final Castle.",
    fields = {
        {
            key = "target_id",
            label = "Target ID",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Triggers Bowser, Dry Bowser and Giant Bowser falling animation.",
        },
        {
            key = "destroys_bridge",
            label = "Destroys bridge",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 1, label = "1 tile below switch" },
                { value = 0, label = "2 tiles below switch" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
