-- Ported from NSMB Editor 5's NSMBStageObj.cs case 68.

local images = {}

local function init(ctx)
    images.HammerBro = ctx.loadImage("images/HammerBro.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 18
    width = 18
    height = 34
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.HammerBro, RenderX, RenderY - 18) or drawn
    return drawn
end

return {
    objectId = 68,
    name = "Hammer Bro.",
    category = "Enemies - Overground",
    notes = "Make sure to put 2 levels of ground at the location of the Hammer Bro, or he will glitch and fall in to the ground.\n\nDoes not have:\n- Any (or at least not any working-) nybbles/values that change the jumping AI, like there are in NSMBW.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
