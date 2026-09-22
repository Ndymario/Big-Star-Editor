-- Ported from NSMB Editor 5's NSMBStageObj.cs case 180.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.SpinningTrianglePlatform = ctx.loadImage("images/SpinningTrianglePlatform.png")
end

local function grabBounds(actor)
    local shift = actor.fields.shift_8_pixels_down and 8 or 0
    return { x = actor.pixelX - 14, y = actor.pixelY - 6 + shift, width = 44, height = 39 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    renderer:draw(images.SpinningTrianglePlatform, box)
    return renderer:draw(images.Clockwise, {
        x = actor.pixelX - 8,
        y = box.y - 2,
        width = 32,
        height = 32,
        flipX = actor.fields.rotation == 0,
    })
end

return {
    objectId = 180,
    name = "Rotating triangle lift",
    category = "Platforms and ropes",
    notes = "\"Only someone with direct access to the database can procede this modification : Class ID is actually 180, not 6 !\"",
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
                { value = 1, label = "120° periodically" },
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
