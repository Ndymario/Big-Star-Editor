-- Ported from NSMB Editor 5's NSMBStageObj.cs case 181.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.SpinningSquarePlatform = ctx.loadImage("images/SpinningSquarePlatform.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 16
    y = y - 16
    if (d[2] & 0x10) // 0x10 == 1 then
        y = y + 8
    end
    width = 48
    height = 48
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local img
    local drawn = false
    if (d[2] & 0x10) // 0x10 == 1 then
        RenderY = RenderY + 8
        RenderY2 = RenderY2 + 8
    end
    drawn = gfx.blit(renderer, images.SpinningSquarePlatform, RenderX - 16, RenderY - 16) or drawn
    img = images.Clockwise
    if d[3] % 0x2 == 0 then
        img = gfx.transform(img, 0, true, false)
    end
    drawn = gfx.blit(renderer, img, RenderX2 - 8, RenderY2 - 8, 32, 32) or drawn
    return drawn
end

return {
    objectId = 181,
    name = "Rotating square lift",
    category = "Platforms and ropes",
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Does not apply if Nybble 9 is set to 2.",
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "spinning",
            label = "Spinning",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Constant" },
                { value = 1, label = "90° periodically" },
                { value = 2, label = "When stood on" },
            },
        },
        {
            key = "shift_block_down",
            label = "Shift ½ block down",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "faster",
            label = "Faster",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "stick_to_a_line",
            label = "Stick to a line",
            bitOffset = 20,
            bitWidth = 4,
            notes = "If you want to go right, put clockwise. If you want to go left, put counterclockwise.",
            checkboxValue = 1,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
