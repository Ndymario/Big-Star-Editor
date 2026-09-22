-- Ported from NSMB Editor 5's NSMBStageObj.cs case 124.

local images = {}

local function init(ctx)
    images.UnderwaterBounceBall = ctx.loadImage("images/UnderwaterBounceBall.png")
end

local function grabBounds(actor)
    local offset = actor.fields.shift_distance * 2
    if actor.fields.shift_left then
        offset = -offset
    end
    return { x = actor.pixelX + offset - 26, y = actor.pixelY - 26, width = 53, height = 53 }
end

local function render(actor, renderer)
    return renderer:draw(images.UnderwaterBounceBall, grabBounds(actor))
end

return {
    objectId = 124,
    name = "Underwater bounce bubble",
    category = "Enemies - Water",
    notes = "If it is ground pounded it will be destroyed, which is a unused behaviour.\n\nBoth event bytes are unused.",
    fields = {
        {
            key = "shift_distance",
            label = "Shift distance",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Unused. Each step shifts the preview by 2 pixels.",
        },
        {
            key = "shift_left",
            label = "Shift left",
            bitOffset = 0,
            bitWidth = 1,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
