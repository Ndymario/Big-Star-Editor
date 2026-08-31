-- Ported from NSMB Editor 5's NSMBStageObj.cs case 169.

local images = {}

local function init(ctx)
    images.HangingPlatform = ctx.loadImage("images/HangingPlatform.png")
    images.HangingPlatformChain = ctx.loadImage("images/HangingPlatformChain.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 192, height = 32 }
end

local function render(actor, renderer)
    local drawn = renderer:draw(images.HangingPlatform, grabBounds(actor))
    for link = 1, 6 do
        drawn = renderer:draw(images.HangingPlatformChain, {
            x = actor.pixelX + 92,
            y = actor.pixelY - 32 * link,
            width = 8,
            height = 32,
        }) or drawn
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
