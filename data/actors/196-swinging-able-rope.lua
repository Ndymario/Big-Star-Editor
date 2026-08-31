-- Ported from NSMB Editor 5's NSMBStageObj.cs case 196.

local images = {}

local function init(ctx)
    images.Rope = ctx.loadImage("images/Rope.png")
end

local function grabBounds(actor)
    local segments = math.min(actor.fields.length_in_tiles, 8)
    return { x = actor.pixelX - 3, y = actor.pixelY, width = 8, height = 16 * segments }
end

local function render(actor, renderer)
    local segments = math.min(actor.fields.length_in_tiles, 8)
    for segment = 0, segments - 1 do
        renderer:draw(images.Rope, { x = actor.pixelX - 3, y = actor.pixelY + 16 * segment })
    end
    return true
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
