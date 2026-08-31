-- Ported from NSMB Editor 5's NSMBStageObj.cs case 265.

local images = {}

local function init(ctx)
    images.BowserSwitch = ctx.loadImage("images/BowserSwitch.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX + 4, y = actor.pixelY - 7, width = 24, height = 23 }
end

local function render(actor, renderer)
    return renderer:draw(images.BowserSwitch, grabBounds(actor))
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
