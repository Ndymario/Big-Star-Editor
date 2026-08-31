-- Ported from NSMB Editor 5's NSMBStageObj.cs case 169.

local images = {}

local function init(ctx)
    images.HangingPlatform = ctx.loadImage("images/HangingPlatform.png")
    images.HangingPlatformChain = ctx.loadImage("images/HangingPlatformChain.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 192
    height = 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.HangingPlatform, RenderX, RenderY) or drawn
    RenderX = RenderX + 92
    for l = 0, (6) - 1 do
        RenderY = RenderY - 32
        drawn = gfx.blit(renderer, images.HangingPlatformChain, RenderX, RenderY) or drawn
    end
    return drawn
end

return {
    objectId = 169,
    name = "Wobbly metal lift",
    category = "Platforms and ropes",
    notes = "Wobbles when the player steps on it. Duh!",
    fields = {
        {
            key = "maximum_tilting",
            label = "Maximum tilting",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 3, label = "7.5°" },
                { value = 2, label = "15°" },
                { value = 1, label = "30°" },
                { value = 0, label = "60°" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
