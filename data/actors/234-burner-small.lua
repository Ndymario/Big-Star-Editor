-- Ported from NSMB Editor 5's NSMBStageObj.cs case 234.

local images = {}

local function init(ctx)
    images.SmallFlamethrower = ctx.loadImage("images/SmallFlamethrower.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.direction == 0 then
        width = 64
    elseif actor.fields.direction == 1 then
        x = x - 48
        width = 64
    elseif actor.fields.direction == 2 then
        y = y - 48
        width = 16
        height = 64
    elseif actor.fields.direction == 3 then
        width = 16
        height = 64
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local direction = actor.fields.direction
    if direction > 3 then
        return false
    end
    if direction == 0 then
        return renderer:draw(images.SmallFlamethrower, { x = actor.pixelX, y = actor.pixelY })
    elseif direction == 1 then
        return renderer:draw(images.SmallFlamethrower, {
            x = actor.pixelX - 48,
            y = actor.pixelY,
            rotation = 180,
        })
    end

    -- gfx.blit's quarter-turn compensation used the unrotated image height in
    -- the up branch. Preserve its low-level command and bounds mismatch.
    return renderer:draw(images.SmallFlamethrower, {
        x = actor.pixelX - 24,
        y = actor.pixelY + 24,
        rotation = direction == 2 and 270 or 90,
    })
end

return {
    objectId = 234,
    name = "Burner (Small)",
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
