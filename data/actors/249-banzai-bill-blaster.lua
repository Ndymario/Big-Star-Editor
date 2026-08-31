-- Ported from NSMB Editor 5's NSMBStageObj.cs case 249.

local images = {}

local function init(ctx)
    images.BanzaiBillCannon = ctx.loadImage("images/BanzaiBillCannon.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 32, y = actor.pixelY - 64, width = 64, height = 80 }
end

local function render(actor, renderer)
    return renderer:draw(images.BanzaiBillCannon, grabBounds(actor))
end

return {
    objectId = 249,
    name = "Banzai Bill Blaster",
    category = "Enemies - Castle",
    notes = "Sprite set: 5-1",
    fields = {
        {
            key = "shooting_direction",
            label = "Shooting direction",
            bitOffset = 8,
            bitWidth = 4,
            notes = "1 and 2 are unused.",
            choices = {
                { value = 1, label = "Left" },
                { value = 2, label = "Right" },
                { value = 0, label = "Both" },
            },
        },
        {
            key = "unknown_11",
            label = "Unknown (11)",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Stored by the game for ulterior use but seems to never be called, still needs testing.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
