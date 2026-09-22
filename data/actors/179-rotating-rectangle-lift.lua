-- Ported from NSMB Editor 5's NSMBStageObj.cs case 179.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.SpinningRectanglePlatform = ctx.loadImage("images/SpinningRectanglePlatform.png")
end

local function grabBounds(actor)
    local shift = actor.fields.shift_8_pixels_down and 8 or 0
    return { x = actor.pixelX - 24, y = actor.pixelY - 8 + shift, width = 64, height = 32 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    renderer:draw(images.SpinningRectanglePlatform, box)
    return renderer:draw(images.Clockwise, {
        x = actor.pixelX - 8,
        y = box.y,
        width = 32,
        height = 32,
        flipX = actor.fields.rotation == 0,
    })
end

return {
    objectId = 179,
    name = "Rotating rectangle lift",
    category = "Platforms and ropes",
    notes = "'Stick to line' option is unused.",
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 16,
            bitWidth = 1,
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
            key = "shift_8_pixels_down",
            label = "Shift 8 pixels down",
            bitOffset = 28,
            bitWidth = 1,
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
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
