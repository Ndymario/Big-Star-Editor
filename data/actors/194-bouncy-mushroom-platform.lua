-- Ported from NSMB Editor 5's NSMBStageObj.cs case 194.

local images = {}

local function init(ctx)
    images.BouncingMushroom = ctx.loadImage("images/BouncingMushroom.png")
    images.BouncingMushroomStalk = ctx.loadImage("images/BouncingMushroomStalk.png")
end

local function grabBounds(actor)
    local shift = 0
    if actor.fields.shift_position == 1 then
        shift = -8
    elseif actor.fields.shift_position == 2 then
        shift = 8
    end
    return {
        x = actor.pixelX + shift - 40,
        y = actor.pixelY,
        width = 96,
        height = 16 * (actor.fields.stalk_height + 2),
    }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    renderer:draw(images.BouncingMushroom, { x = box.x, y = box.y })
    for segment = 0, actor.fields.stalk_height - 1 do
        renderer:draw(images.BouncingMushroomStalk, {
            x = box.x + 43,
            y = box.y + 32 + 16 * segment,
        })
    end
    return true
end

return {
    objectId = 194,
    name = "Bouncy Mushroom Platform",
    category = "Platforms and ropes",
    notes = "No problems with underwater players bouncing on it, but doesn't appear to have special underwater behaviour.",
    fields = {
        {
            key = "stalk_height",
            label = "Stalk height",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "shift_position",
            label = "Shift position",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "No shifting" },
                { value = 1, label = "8 pixels left" },
                { value = 2, label = "8 pixels right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
