-- Ported from NSMB Editor 5's NSMBStageObj.cs case 256.

local images = {}

local function init(ctx)
    images.SpinBoard = ctx.loadImage("images/SpinBoard.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 6, y = actor.pixelY, width = 26, height = 26 }
end

local function render(actor, renderer)
    return renderer:draw(images.SpinBoard, grabBounds(actor))
end

return {
    objectId = 256,
    name = "Spin Block (unused Actor 256)",
    category = "Platforms and ropes",
    notes = "Not complete because: Unknown values, unknown use\nWon't appear if Sprite 95 is not in immediate vicinity.",
    fields = {
        {
            key = "unknown_8",
            label = "Unknown 8",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Used in-game. Doesn't do anything notable.",
        },
        {
            key = "unknown_9",
            label = "Unknown 9",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Used in-game. Doesn't do anything notable.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
