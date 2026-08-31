-- Ported from NSMB Editor 5's NSMBStageObj.cs case 41.

local images = {}

local function init(ctx)
    images.Boo = ctx.loadImage("images/Boo.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 24
    height = 20
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Boo, RenderX, RenderY) or drawn
    return drawn
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
