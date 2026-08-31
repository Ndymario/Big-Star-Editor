-- Ported from NSMB Editor 5's NSMBStageObj.cs case 90.

local images = {}

local function init(ctx)
    images.Swooper = ctx.loadImage("images/Swooper.png")
    images.SwooperLarge = ctx.loadImage("images/SwooperLarge.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if d[5] % 0x10 == 1 then
        x = x - 6
        y = y - 16
        width = 26
        height = 37
    else
        height = 18
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    if d[5] % 0x10 == 1 then
        drawn = gfx.blit(renderer, images.SwooperLarge, RenderX - 6, RenderY - 16) or drawn
    else
        drawn = gfx.blit(renderer, images.Swooper, RenderX, RenderY) or drawn
    end
    return drawn
end

return {
    objectId = 90,
    name = "Swooper",
    category = "Enemies - Underground",
    notes = "A bat.",
    fields = {
        {
            key = "size",
            label = "Size",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Somewhat buggy when putting values between 2 and 17 inclusive as the sprite and the hitbox become completely disjointed.",
            choices = {
                { value = 0, label = "Small" },
                { value = 1, label = "Big" },
                { value = 32, label = "Small (shift 1 tile down)" },
                { value = 33, label = "Big (shift 2 tiles down)" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
