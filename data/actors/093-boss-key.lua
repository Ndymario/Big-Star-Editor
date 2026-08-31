-- Ported from NSMB Editor 5's NSMBStageObj.cs case 93.

local images = {}

local function init(ctx)
    images.BossKey = ctx.loadImage("images/BossKey.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 20
    y = y - 15
    width = 25
    height = 31
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.BossKey, RenderX - 20, RenderY - 15) or drawn
    return drawn
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
