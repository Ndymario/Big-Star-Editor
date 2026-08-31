-- Ported from NSMB Editor 5's NSMBStageObj.cs case 235.

local images = {}

local function init(ctx)
    images.Flamethrower = ctx.loadImage("images/Flamethrower.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.direction == 0 then
        width = 128
    elseif actor.fields.direction == 1 then
        x = x - 112
        width = 128
    elseif actor.fields.direction == 2 then
        y = y - 112
        width = 16
        height = 128
    elseif actor.fields.direction == 3 then
        width = 16
        height = 128
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local direction = actor.fields.direction
    if direction > 3 then
        return false
    end
    if direction == 0 then
        return renderer:draw(images.Flamethrower, { x = actor.pixelX, y = actor.pixelY })
    elseif direction == 1 then
        return renderer:draw(images.Flamethrower, {
            x = actor.pixelX - 112,
            y = actor.pixelY,
            rotation = 180,
        })
    end

    -- gfx.blit's quarter-turn compensation used the unrotated image height in
    -- the up branch. Preserve its low-level command and bounds mismatch.
    return renderer:draw(images.Flamethrower, {
        x = actor.pixelX - 56,
        y = actor.pixelY + 56,
        rotation = direction == 2 and 270 or 90,
    })
end

return {
    objectId = 235,
    name = "Burner",
    category = "Enemies - Castle",
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 6,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
                { value = 2, label = "Up" },
                { value = 3, label = "Down" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
