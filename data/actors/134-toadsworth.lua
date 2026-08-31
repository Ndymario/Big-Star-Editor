-- Ported from NSMB Editor 5's NSMBStageObj.cs case 134.

local images = {}

local function init(ctx)
    images.ToadHouse1UP = ctx.loadImage("images/ToadHouse1UP.png")
    images.ToadHouseItem = ctx.loadImage("images/ToadHouseItem.png")
    images.ToadHouseMega = ctx.loadImage("images/ToadHouseMega.png")
    images.Toadsworth = ctx.loadImage("images/Toadsworth.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 15
    width = 20
    height = 31
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Toadsworth, RenderX, RenderY - 15) or drawn
    if actor.fields.type == 0 then
        drawn = gfx.blit(renderer, images.ToadHouse1UP, RenderX - 188, RenderY - 88) or drawn
    end
    if actor.fields.type == 2 then
        drawn = gfx.blit(renderer, images.ToadHouseItem, RenderX - 116, RenderY - 76) or drawn
    end
    if actor.fields.type == 3 then
        drawn = gfx.blit(renderer, images.ToadHouseMega, RenderX - 116, RenderY - 76) or drawn
    end
    return drawn
end

return {
    objectId = 134,
    name = "Toadsworth",
    category = "Bosses and cutscenes",
    fields = {
        {
            key = "type",
            label = "Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1 makes glitchy blocks that do nothing. Never use it.",
            choices = {
                { value = 0, label = "1UP" },
                { value = 1, label = "Enemy" },
                { value = 2, label = "Item" },
                { value = 3, label = "Mega mushroom" },
                { value = 4, label = "Background chooser" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
