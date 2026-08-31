-- Ported from NSMB Editor 5's NSMBStageObj.cs case 262.

local images = {}

local function init(ctx)
    images.RotatingCannon = ctx.loadImage("images/RotatingCannon.png")
    images.RotatingCannonEmpty = ctx.loadImage("images/RotatingCannonEmpty.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 3,
        y = actor.pixelY - 16 * (actor.fields.height - 1),
        width = 23,
        height = 16 * actor.fields.height,
    }
end

local function render(actor, renderer)
    local drawn = false
    for level = 0, actor.fields.height - 1 do
        local y = actor.pixelY - 16 * level
        local bit = 1 << level
        if (actor.fields.empty_spots & bit) > 0 then
            drawn = renderer:draw(images.RotatingCannonEmpty, { x = actor.pixelX + 2, y = y }) or drawn
        elseif (actor.fields.cannons_to_flip & bit) > 0 then
            drawn = renderer:draw(images.RotatingCannon, {
                x = actor.pixelX - 3,
                y = y,
                flipX = true,
            }) or drawn
        else
            drawn = renderer:draw(images.RotatingCannon, { x = actor.pixelX + 1, y = y }) or drawn
        end
    end
    return drawn
end

return {
    objectId = 262,
    name = "Bill Blaster Turret (Direction changing Bullet Bill launcher)",
    category = "Enemies - Castle",
    notes = "First checkboxes are bottom cannons.",
    fields = {
        {
            key = "empty_spots",
            label = "Empty spots",
            bitOffset = 8,
            bitWidth = 8,
            notes = "individual bits indicate empty spots",
        },
        {
            key = "cannons_to_flip",
            label = "Cannons to flip",
            bitOffset = 0,
            bitWidth = 8,
            notes = "individual bits indicate flipped cannons",
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 16,
            bitWidth = 3,
            notes = "Height is 1 + (N7 mod 8)",
            valueOffset = 1,
        },
        {
            key = "delayed_firing",
            label = "Delayed firing",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
