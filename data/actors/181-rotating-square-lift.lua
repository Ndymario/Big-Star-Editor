-- Ported from NSMB Editor 5's NSMBStageObj.cs case 181.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.SpinningSquarePlatform = ctx.loadImage("images/SpinningSquarePlatform.png")
end

local function grabBounds(actor)
    local shift = actor.fields.shift_8_pixels_down and 8 or 0
    return { x = actor.pixelX - 16, y = actor.pixelY - 16 + shift, width = 48, height = 48 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    renderer:draw(images.SpinningSquarePlatform, box)
    return renderer:draw(images.Clockwise, {
        x = actor.pixelX - 8,
        y = box.y + 8,
        width = 32,
        height = 32,
        flipX = actor.fields.rotation == 0,
    })
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
