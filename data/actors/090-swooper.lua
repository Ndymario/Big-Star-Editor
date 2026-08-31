-- Ported from NSMB Editor 5's NSMBStageObj.cs case 90.

local images = {}

local function init(ctx)
    images.Swooper = ctx.loadImage("images/Swooper.png")
    images.SwooperLarge = ctx.loadImage("images/SwooperLarge.png")
end

local function grabBounds(actor)
    if actor.fields.size == 1 then
        return { x = actor.pixelX - 6, y = actor.pixelY - 16, width = 26, height = 37 }
    end
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 18 }
end

local function render(actor, renderer)
    local image = actor.fields.size == 1 and images.SwooperLarge or images.Swooper
    return renderer:draw(image, grabBounds(actor))
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
            bitWidth = 4,
            notes = "Somewhat buggy when putting values between 2 and 17 inclusive as the sprite and the hitbox become completely disjointed.",
            choices = {
                { value = 0, label = "Small" },
                { value = 1, label = "Big" },
            },
        },
        {
            key = "spawn_shift",
            label = "Spawn shift",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "None" },
                { value = 2, label = "Shift down" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
