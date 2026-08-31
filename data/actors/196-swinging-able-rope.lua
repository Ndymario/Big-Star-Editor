-- Ported from NSMB Editor 5's NSMBStageObj.cs case 196.

local images = {}

local function init(ctx)
    images.Rope = ctx.loadImage("images/Rope.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    local RopeH = math.min(d[5] % 0x10, 4)
    x = x - 3
    width = 8
    height = 16 * (RopeH + 4)
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    local RopeMax = math.min(d[5] % 0x10, 4)
    for l = 0, (RopeMax + 4) - 1 do
        drawn = gfx.blit(renderer, images.Rope, RenderX - 3, RenderY) or drawn
        RenderY = RenderY + 16
    end
    return drawn
end

return {
    objectId = 196,
    name = "Swinging/able rope",
    category = "Platforms and ropes",
    fields = {
        {
            key = "mario_controls_sway",
            label = "Mario controls sway",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "length_in_tiles",
            label = "Length in tiles",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Maximum = 8.",
            valueOffset = 4,
        },
        {
            key = "sway_on_own",
            label = "Sway on own +/-",
            bitOffset = 4,
            bitWidth = 4,
            notes = "These values go from 0 (none) to F (almost horizontal). At a 4 tile (0 value length) they increase 1/2 tile at the bottom per number.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
