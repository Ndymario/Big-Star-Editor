-- Ported from NSMB Editor 5's NSMBStageObj.cs case 189.

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
        x = actor.pixelX - 16 * (actor.fields.length + 1),
        y = actor.pixelY,
        width = 32 * (actor.fields.length + 2),
        height = 16 * (actor.fields.stalk_height + 4),
    }
end

local function render(actor, renderer)
    local stalkX = actor.pixelX + 6
    renderer:draw(images.MushroomStalkTop, { x = stalkX, y = actor.pixelY + 24 })
    for segment = 0, actor.fields.stalk_height + 1 do
        renderer:draw(images.MushroomStalk, { x = stalkX, y = actor.pixelY + 32 + 16 * segment })
    end

    local edge = actor.fields.color == 1 and images.MushroomEdgeGreen or images.MushroomEdge
    local section = actor.fields.color == 1 and images.MushroomSectionGreen or images.MushroomSection
    local box = grabBounds(actor)
    renderer:draw(edge, { x = box.x, y = box.y })
    for column = 1, actor.fields.length do
        renderer:draw(section, { x = box.x + 32 * column, y = box.y })
    end
    return renderer:draw(edge, {
        x = box.x + 32 * (actor.fields.length + 1),
        y = box.y,
        flipX = true,
    })
end

return {
    objectId = 189,
    name = "Mushroom Platform lift (tilting and bobbing)",
    category = "Platforms and ropes",
    notes = "\n\t\t\t\tA basic mushroom platform, where Mario/Luigi can stand. There is unused stuff like moving when stood on (huh? -Hiccup). 7 is the maximum height that works with sprite data.  If you want it to be longer, set it to 7 and then use a mushroom stalk object in the tileset to make the mushroom's stalk height look higher.\n\t\t\t\tThis mushroom can be either red or green.\n\t\t\t",
    fields = {
        {
            key = "bobbing",
            label = "Bobbing",
            bitOffset = 21,
            bitWidth = 1,
            checkboxValue = 1,
        },
        {
            key = "color",
            label = "Color",
            bitOffset = 23,
            bitWidth = 1,
            notes = "Green is unused.",
            choices = {
                { value = 0, label = "Red" },
                { value = 1, label = "Green" },
            },
        },
        {
            key = "stalk_height",
            label = "Stalk height",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Max 7. Add tiles under it.",
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 12,
            bitWidth = 3,
        },
        {
            key = "tilt_angle",
            label = "Tilt angle",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 0,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
