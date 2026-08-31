-- Ported from NSMB Editor 5's NSMBStageObj.cs case 213.

local images = {}

local function init(ctx)
    images.OneWayDoor = ctx.loadImage("images/OneWayDoor.png")
end

local function grabBounds(actor)
    local d = actor.data
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
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.OneWayDoor
    if d[5] == 0 or d[5] == 1 then
        img = gfx.transform(img, 0, false, true)
        RenderY = RenderY - 39
    elseif d[5] == 2 or d[5] == 3 then
        RenderY = RenderY + 16
    elseif d[5] == 4 or d[5] == 5 then
        img = gfx.transform(img, 270, false, false)
        RenderX = RenderX + 16
    elseif d[5] == 6 or d[5] == 7 then
        img = gfx.transform(img, 270, true, false)
        RenderX = RenderX - 39
    end
    if d[5] == 0 or d[5] == 2 then
        img = gfx.transform(img, 0, true, false)
    elseif d[5] == 4 or d[5] == 6 then
        img = gfx.transform(img, 0, false, true)
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
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
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
