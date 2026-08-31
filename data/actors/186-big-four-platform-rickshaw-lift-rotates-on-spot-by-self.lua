-- Ported from NSMB Editor 5's NSMBStageObj.cs case 186.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.GiantSpinningPlatform = ctx.loadImage("images/GiantSpinningPlatform.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 144, y = actor.pixelY - 104, width = 288, height = 208 }
end

local function render(actor, renderer)
    renderer:draw(images.GiantSpinningPlatform, grabBounds(actor))
    return renderer:draw(images.Clockwise, {
        x = actor.pixelX - 16,
        y = actor.pixelY - 16,
        width = 32,
        height = 32,
        flipX = actor.fields.rotation == 0,
    })
end

return {
    objectId = 186,
    name = "Big four platform rickshaw lift (rotates on spot by self)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 16,
            bitWidth = 1,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "enable_trigger_id_check",
            label = "Enable trigger ID check",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Unused.",
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
            notes = "Unused.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
