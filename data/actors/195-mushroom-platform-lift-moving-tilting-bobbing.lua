-- Ported from NSMB Editor 5's NSMBStageObj.cs case 195.

local images = {}

local function init(ctx)
    images.MushroomEdge = ctx.loadImage("images/MushroomEdge.png")
    images.MushroomEdgeGreen = ctx.loadImage("images/MushroomEdgeGreen.png")
    images.MushroomSection = ctx.loadImage("images/MushroomSection.png")
    images.MushroomSectionGreen = ctx.loadImage("images/MushroomSectionGreen.png")
    images.MushroomStalk = ctx.loadImage("images/MushroomStalk.png")
    images.MushroomStalkTop = ctx.loadImage("images/MushroomStalkTop.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 16 * (actor.fields.width + 1),
        y = actor.pixelY,
        width = 32 * (actor.fields.width + 2),
        height = 64,
    }
end

local function render(actor, renderer)
    local stalkX = actor.pixelX + 6
    renderer:draw(images.MushroomStalkTop, { x = stalkX, y = actor.pixelY + 24 })
    renderer:draw(images.MushroomStalk, { x = stalkX, y = actor.pixelY + 32 })
    renderer:draw(images.MushroomStalk, { x = stalkX, y = actor.pixelY + 48 })

    local edge = actor.fields.color == 1 and images.MushroomEdgeGreen or images.MushroomEdge
    local section = actor.fields.color == 1 and images.MushroomSectionGreen or images.MushroomSection
    local box = grabBounds(actor)
    renderer:draw(edge, { x = box.x, y = box.y })
    for column = 1, actor.fields.width do
        renderer:draw(section, { x = box.x + 32 * column, y = box.y })
    end
    return renderer:draw(edge, {
        x = box.x + 32 * (actor.fields.width + 1),
        y = box.y,
        flipX = true,
    })
end

return {
    objectId = 195,
    name = "Mushroom Platform lift (Moving, tilting & bobbing)",
    category = "Platforms and ropes",
    notes = "\n\t\t\t\tMoves to the right until it hits a wall, then goes left, until it hits a wall, ect\n\t\t\t\tThis mushroom can be either red or green.\n\t\t\t",
    fields = {
        {
            key = "use_tracks",
            label = "Use tracks",
            bitOffset = 28,
            bitWidth = 1,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "horizontal_speed",
            label = "Horizontal speed",
            bitOffset = 24,
            bitWidth = 4,
        },
        {
            key = "color",
            label = "Color",
            bitOffset = 20,
            bitWidth = 1,
            notes = "Green is unused.",
            choices = {
                { value = 0, label = "Red" },
                { value = 1, label = "Green" },
            },
        },
        {
            key = "stalk_height_shift",
            label = "Stalk Height/Shift",
            bitOffset = 16,
            bitWidth = 4,
        },
        {
            key = "width",
            label = "Width",
            bitOffset = 12,
            bitWidth = 3,
        },
        {
            key = "tilting_angle",
            label = "Tilting angle",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "bop_speed",
            label = "Bop speed",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "tilting_speed",
            label = "Tilting speed",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
