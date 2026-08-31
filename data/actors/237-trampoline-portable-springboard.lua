-- Ported from NSMB Editor 5's NSMBStageObj.cs case 237.

local images = {}

local function init(ctx)
    images.Spring = ctx.loadImage("images/Spring.png")
    images.SpringGiant = ctx.loadImage("images/SpringGiant.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.larger then
        x = x - 8
        y = y - 16
        width = 32
        height = 32
    end
    if actor.fields.shift_block_right then
        x = x + 8
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    if actor.fields.larger then
        img = images.SpringGiant
        RenderY = RenderY - 16
        RenderX = RenderX - 8
    else
        img = images.Spring
    end
    if actor.fields.shift_block_right then
        RenderX = RenderX + 8
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 237,
    name = "Trampoline (portable springboard)",
    category = "Platforms and ropes",
    notes = "An item used to make Mario jump higher. Can be carried by Mario. Objects can push it (but only in later versions?).\n\nBounces the player up by 6 tiles (11 if mini).\n\nThe big version bounces the player up by 18 tiles (29 if mini).\n\nThe pickup hitbox for the giant springboard is far too large and extends even above it, which makes you pick it up if you're holding Y and don't press B in time when springing off it. (hitbox information & text: skawo)",
    fields = {
        {
            key = "shift_block_right",
            label = "Shift ½ block right",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "larger",
            label = "Larger",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused. (Can get stuck in walls)",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
