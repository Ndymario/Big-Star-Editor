-- Ported from NSMB Editor 5's NSMBStageObj.cs case 202.

local images = {}

local function init(ctx)
    images.Pipe = ctx.loadImage("images/Pipe.png")
    images.PipeLeft = ctx.loadImage("images/PipeLeft.png")
    images.PipeLongRight = ctx.loadImage("images/PipeLongRight.png")
    images.PipeRight = ctx.loadImage("images/PipeRight.png")
    images.PipeShortLeft = ctx.loadImage("images/PipeShortLeft.png")
    images.PipeShortRight = ctx.loadImage("images/PipeShortRight.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if d[5] % 0x8 == 0 then
        x = x + 1
        y = y - 49
        width = 55
        height = 65
    elseif d[5] % 0x8 == 1 then
        x = x - 23
        y = y - 49
        width = 55
        height = 65
    elseif d[5] % 0x8 == 2 then
        x = x + 1
        y = y - 50
        width = 40
        height = 66
    elseif d[5] % 0x8 == 3 then
        x = x - 8
        y = y - 50
        width = 40
        height = 66
    elseif d[5] % 0x8 == 5 then
        x = x + 1
        y = y - 47
        width = 57
        height = 63
    else
        y = y - 48
        width = 32
        height = 64
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    if d[5] % 0x8 == 0 then
        img = images.PipeRight
        RenderX = RenderX + 1
        RenderY = RenderY - 1
    elseif d[5] % 0x8 == 1 then
        img = images.PipeLeft
        RenderX = RenderX - 23
        RenderY = RenderY - 1
    elseif d[5] % 0x8 == 2 then
        img = images.PipeShortRight
        RenderX = RenderX + 1
        RenderY = RenderY - 2
    elseif d[5] % 0x8 == 3 then
        img = images.PipeShortLeft
        RenderX = RenderX - 8
        RenderY = RenderY - 2
    elseif d[5] % 0x8 == 5 then
        img = images.PipeLongRight
        RenderX = RenderX + 1
        RenderY = RenderY + 1
    else
        img = images.Pipe
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY - 48) or drawn
    return drawn
end

return {
    objectId = 202,
    name = "Pipe Cannon",
    category = "Platforms and ropes",
    fields = {
        {
            key = "angle_force",
            label = "Angle / Force",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1 is unused.",
            choices = {
                { value = 0, label = "Far right" },
                { value = 1, label = "Far left" },
                { value = 2, label = "Medium right" },
                { value = 3, label = "Medium left" },
                { value = 4, label = "Straight up" },
                { value = 5, label = "Very far right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
