-- Ported from NSMB Editor 5's NSMBStageObj.cs case 41.

local images = {}

local function init(ctx)
    images.Boo = ctx.loadImage("images/Boo.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 24, height = 20 }
end

local function render(actor, renderer)
    return renderer:draw(images.Boo, grabBounds(actor))
end

return {
    objectId = 41,
    name = "Boo",
    category = "Enemies - Ghost House",
    notes = "This sprite will freeze and remain frozen indefinitely if placed in a view with music that includes a \"bahp\" note, such as music IDs 1A and 63.",
    fields = {
        {
            key = "faster",
            label = "Faster",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
