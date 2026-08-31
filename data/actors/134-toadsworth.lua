-- Ported from NSMB Editor 5's NSMBStageObj.cs case 134.

local images = {}

local function init(ctx)
    images.ToadHouse1UP = ctx.loadImage("images/ToadHouse1UP.png")
    images.ToadHouseItem = ctx.loadImage("images/ToadHouseItem.png")
    images.ToadHouseMega = ctx.loadImage("images/ToadHouseMega.png")
    images.Toadsworth = ctx.loadImage("images/Toadsworth.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 15, width = 20, height = 31 }
end

local function render(actor, renderer)
    local drawn = renderer:draw(images.Toadsworth, grabBounds(actor))
    if actor.fields.type == 0 then
        return renderer:draw(images.ToadHouse1UP, {
            x = actor.pixelX - 188, y = actor.pixelY - 88, width = 184, height = 31,
        }) or drawn
    elseif actor.fields.type == 2 then
        return renderer:draw(images.ToadHouseItem, {
            x = actor.pixelX - 116, y = actor.pixelY - 76, width = 40, height = 40,
        }) or drawn
    elseif actor.fields.type == 3 then
        return renderer:draw(images.ToadHouseMega, {
            x = actor.pixelX - 116, y = actor.pixelY - 76, width = 40, height = 40,
        }) or drawn
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
