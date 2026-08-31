-- Ported from NSMB Editor 5's NSMBStageObj.cs case 66.

local images = {}

local function init(ctx)
    images.Coin = ctx.loadImage("images/Coin.png")
    images.CoinInBubble = ctx.loadImage("images/CoinInBubble.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    if actor.fields.type == 1 then
        return { x = x - 4, y = y - 4, width = 24, height = 24 }
    elseif actor.fields.type == 2 then
        return { x = x - 4, y = y - 20, width = 72, height = 40 }
    end
    if actor.fields.shift_8_pixels_down then y = y + 8 end
    if actor.fields.shift_8_pixels_right then x = x + 8 end
    return { x = x, y = y, width = 16, height = 16 }
end

local function render(actor, renderer)
    if actor.fields.type == 1 then
        return renderer:draw(images.CoinInBubble, grabBounds(actor))
    elseif actor.fields.type == 2 then
        local drawn = renderer:draw(images.CoinInBubble, {
            x = actor.pixelX - 4, y = actor.pixelY - 4, width = 24, height = 24,
        })
        drawn = renderer:draw(images.CoinInBubble, {
            x = actor.pixelX + 20, y = actor.pixelY - 20, width = 24, height = 24,
        }) or drawn
        return renderer:draw(images.CoinInBubble, {
            x = actor.pixelX + 44, y = actor.pixelY - 4, width = 24, height = 24,
        }) or drawn
    end
    return renderer:draw(images.Coin, grabBounds(actor))
end

return {
    objectId = 66,
    name = "Coin",
    category = "Uncategorized",
    notes = "This is an actor version of the coin. Because it is an actor, you can do more things with it.",
    fields = {
        {
            key = "unk_4",
            label = "Unk 4",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Set to 1 in coin thrown out by unused ? block that throws out 1 coin.",
        },
        {
            key = "shift_8_pixels_right",
            label = "Shift 8 pixels right",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "shift_8_pixels_down",
            label = "Shift 8 pixels down",
            bitOffset = 20,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "direction_z_order",
            label = "Direction/Z order",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Left/in front" },
                { value = 1, label = "Left down/behind" },
                { value = 2, label = "Left/more down" },
                { value = 7, label = "Left down slow" },
                { value = 10, label = "Right" },
                { value = 14, label = "Up right fast" },
            },
        },
        {
            key = "unk_10",
            label = "Unk 10",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Used by unused coin spawner.",
        },
        {
            key = "type",
            label = "Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "3 is used when you defeat a Whomp, Big Piranha Plant or Moneybag. 5 is used when you fireball an enemy like a goomba. 11 is used when you defeat a chain chomp. 7 is used for the unused floating coin ? block. 10 is used by unused ? block that throws out 1 coin. 4 is used by the unused coin spawner.",
            choices = {
                { value = 0, label = "Regular" },
                { value = 1, label = "In bubble" },
                { value = 2, label = "Three in bubble" },
                { value = 3, label = "Hops up-left" },
                { value = 4, label = "Drops" },
                { value = 5, label = "Hops" },
                { value = 6, label = "Moves right" },
                { value = 7, label = "Moves up-left" },
                { value = 8, label = "Coin Cheep coin" },
                { value = 10, label = "Unk 10" },
                { value = 11, label = "Hops high up-left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
