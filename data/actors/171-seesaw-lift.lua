-- Ported from NSMB Editor 5's NSMBStageObj.cs case 171.

local images = {}

local function init(ctx)
    images.SeeSaw = ctx.loadImage("images/SeeSaw.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 256, height = 16 }
end

local function render(actor, renderer)
    return renderer:draw(images.SeeSaw, grabBounds(actor))
end

return {
    objectId = 171,
    name = "Seesaw lift",
    category = "Platforms and ropes",
    notes = "A.K.A\n* See-saw Platform",
    fields = {
        {
            key = "0_disable_1_enable_trigger_id_check",
            label = "0=Disable/1=Enable Trigger ID check",
            bitOffset = 28,
            bitWidth = 4,
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
