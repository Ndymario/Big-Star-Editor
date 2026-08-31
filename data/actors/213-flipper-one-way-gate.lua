-- Ported from NSMB Editor 5's NSMBStageObj.cs case 213.

local images = {}

-- gfx.transform replaces an earlier transform instead of composing it. These
-- are the exact low-level commands emitted by the original two-pass logic.
local renderGeometry = {
    [0] = { x = 0, y = -39, flipX = true },
    [1] = { x = 0, y = -39, flipY = true },
    [2] = { x = 0, y = 16, flipX = true },
    [3] = { x = 0, y = 16 },
    [4] = { x = 16, y = 0, flipY = true },
    [5] = { x = 27.5, y = -11.5, rotation = 270 },
    [6] = { x = -39, y = 0, flipY = true },
    [7] = { x = -27.5, y = -11.5, rotation = 270, flipX = true },
}

local function init(ctx)
    images.OneWayDoor = ctx.loadImage("images/OneWayDoor.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    local nvalue = actor.fields.type
    if nvalue == 0 or nvalue == 1 then
        y = y - 39
        width = 16
        height = 39
    elseif nvalue == 2 or nvalue == 3 then
        y = y + 16
        width = 16
        height = 39
    elseif nvalue == 4 or nvalue == 5 then
        x = x + 16
        width = 39
        height = 16
    elseif nvalue == 6 or nvalue == 7 then
        x = x - 39
        width = 39
        height = 16
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local g
    if actor.fields.unknown_10 == 0 then
        g = renderGeometry[actor.fields.type]
    end
    g = g or { x = 0, y = 0 }
    return renderer:draw(images.OneWayDoor, {
        x = actor.pixelX + g.x,
        y = actor.pixelY + g.y,
        rotation = g.rotation,
        flipX = g.flipX,
        flipY = g.flipY,
    })
end

return {
    objectId = 213,
    name = "Flipper (one-way gate)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "type",
            label = "Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Nybble 8+ is 2 tiles higher opens from left, and can't be touched by Mario.",
            choices = {
                { value = 0, label = "Faces up; opens from left" },
                { value = 1, label = "Faces up; opens from right" },
                { value = 2, label = "Faces down; opens from left" },
                { value = 3, label = "Faces down; opens from right" },
                { value = 4, label = "Faces right; opens from bottom" },
                { value = 5, label = "Faces right; opens from top" },
                { value = 6, label = "Faces left; opens from bottom" },
                { value = 7, label = "Faces left; opens from top" },
            },
        },
        {
            key = "unknown_10",
            label = "Unknown 10",
            bitOffset = 4,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
