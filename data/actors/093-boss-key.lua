-- Ported from NSMB Editor 5's NSMBStageObj.cs case 93.

local images = {}

local function init(ctx)
    images.BossKey = ctx.loadImage("images/BossKey.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 20, y = actor.pixelY - 15, width = 25, height = 31 }
end

local function render(actor, renderer)
    return renderer:draw(images.BossKey, { x = actor.pixelX - 20, y = actor.pixelY - 15 })
end

return {
    objectId = 93,
    name = "Boss Key",
    category = "Bosses and cutscenes",
    notes = "Spawned by boss(related) actors.",
    fields = {
        {
            key = "blocks_horiz_scroll",
            label = "Blocks horiz. scroll",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Not necessarily useful, (and slightly glichy) but it scrolls the camera the defined amount of pixels. high numbers crash the game",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
