-- Ported from NSMB Editor 5's NSMBStageObj.cs case 147.

local images = {}

local function init(ctx)
    images.SpinyBeetleDown = ctx.loadImage("images/SpinyBeetleDown.png")
    images.SpinyBeetleLeft = ctx.loadImage("images/SpinyBeetleLeft.png")
    images.SpinyBeetleRight = ctx.loadImage("images/SpinyBeetleRight.png")
    images.SpinyBeetleUp = ctx.loadImage("images/SpinyBeetleUp.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if d[5] // 0x10 == 1 then
        x = x - 4
        width = 20
    elseif d[5] // 0x10 == 2 then
        height = 20
    elseif d[5] // 0x10 == 3 then
        width = 20
    else
        y = y - 4
        height = 20
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    if d[5] // 0x10 == 1 then
        drawn = gfx.blit(renderer, images.SpinyBeetleLeft, RenderX - 4, RenderY) or drawn
    elseif d[5] // 0x10 == 2 then
        drawn = gfx.blit(renderer, images.SpinyBeetleDown, RenderX, RenderY) or drawn
    elseif d[5] // 0x10 == 3 then
        drawn = gfx.blit(renderer, images.SpinyBeetleRight, RenderX, RenderY) or drawn
    else
        drawn = gfx.blit(renderer, images.SpinyBeetleUp, RenderX, RenderY - 4) or drawn
    end
    return drawn
end

return {
    objectId = 147,
    name = "Spike Top",
    category = "Enemies - Underground",
    notes = "A beetle with a spike on its head, that crawls slowly on all sides of a surface.\n\nUnused behaviours:\n- Can be defeated by Drill Stomp [may be spiny leftover?]",
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 8,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
                { value = 16, label = "Up (spike on left)" },
                { value = 17, label = "Down (spike on right)" },
                { value = 32, label = "Left (upside down)" },
                { value = 33, label = "Right (upside down)" },
                { value = 48, label = "Down (spike on left)" },
                { value = 49, label = "Up (spike on right)" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
