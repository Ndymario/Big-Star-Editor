-- Ported from NSMB Editor 5's NSMBStageObj.cs case 95.

local images = {}

local function init(ctx)
    images.ParakoopaBlue = ctx.loadImage("images/ParakoopaBlue.png")
    images.ParakoopaGreen = ctx.loadImage("images/ParakoopaGreen.png")
    images.ParakoopaRed = ctx.loadImage("images/ParakoopaRed.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 16, width = 19, height = 32 }
end

local function render(actor, renderer)
    local image
    if actor.fields.color == 1 then
        image = images.ParakoopaRed
    elseif actor.fields.color == 2 then
        image = images.ParakoopaBlue
    else
        image = images.ParakoopaGreen
    end
    return renderer:draw(image, grabBounds(actor))
end

return {
    objectId = 95,
    name = "Koopa Paratroopa",
    category = "Enemies - Overground",
    notes = "Unused behaviours:\n- Has water behaviour when Direction is set to Jumping [may be used, or koopa leftover]",
    fields = {
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 12,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Down/Left" },
                { value = 1, label = "Up/Right" },
            },
        },
        {
            key = "starting_position",
            label = "Starting position",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Midway" },
                { value = 1, label = "Start Bottom-Left" },
                { value = 2, label = "Start Top-Right" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Jumping" },
                { value = 1, label = "Left-right" },
                { value = 2, label = "Up-down" },
            },
        },
        {
            key = "color",
            label = "Color",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Red Koopas stays on ledges.",
            choices = {
                { value = 0, label = "Green" },
                { value = 1, label = "Red" },
                { value = 2, label = "Blue" },
                { value = 3, label = "Blue (stays on ledges)" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
