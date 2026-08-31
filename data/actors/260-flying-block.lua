-- Ported from NSMB Editor 5's NSMBStageObj.cs case 260.

local images = {}

local function init(ctx)
    images.FlyingQBlock = ctx.loadImage("images/FlyingQBlock.png")
    images.FlyingQBlockOverrides = ctx.loadImage("images/FlyingQBlockOverrides.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 9
    y = y - 13
    width = 38
    height = 28
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local width, height = 0, 0
    local drawn = false
    drawn = gfx.blit(renderer, images.FlyingQBlock, RenderX - 9, RenderY - 13) or drawn
    if actor.fields.item < 8 then
        drawn = gfx.blit(renderer, images.FlyingQBlockOverrides, RenderX, RenderY, nil, nil, { x = (actor.fields.item) * 16, y = 0, width = 16, height = 16 }) or drawn
    end
    return drawn
end

return {
    objectId = 260,
    name = "Flying ? Block",
    category = "Event controllers",
    fields = {
        {
            key = "item",
            label = "Item",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Spawned switches are multiple use and physics-affected. Nybble 7 doesn't work with Luigi.",
            choices = {
                { value = 0, label = "Coin" },
                { value = 1, label = "Mushroom/flower" },
                { value = 2, label = "Starman" },
                { value = 3, label = "1UP" },
                { value = 4, label = "? switch" },
                { value = 5, label = "P-switch" },
                { value = 6, label = "Red ! switch" },
                { value = 7, label = "Vine with Mario clone" },
                { value = 8, label = "Mini mushroom" },
            },
        },
        {
            key = "starting_position",
            label = "Starting position",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Left" },
                { value = 1, label = "Right" },
            },
        },
        {
            key = "looping_pattern",
            label = "Looping pattern",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Fly off screen" },
                { value = 1, label = "Center of loop" },
                { value = 2, label = "Edge of loop" },
            },
        },
        {
            key = "flying_pattern",
            label = "Flying pattern",
            bitOffset = 12,
            bitWidth = 4,
            choices = {
                { value = 0, label = "16 tile" },
                { value = 1, label = "Forward 4/back 8" },
                { value = 2, label = "Stay in place" },
            },
        },
        {
            key = "gets_pointed_to_by_phantom_hands",
            label = "Gets pointed to by Phantom Hands",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 1, label = "Yes" },
                { value = 0, label = "No" },
            },
        },
        {
            key = "phantom_hand_id",
            label = "Phantom Hand ID",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Pointing hands only.",
        },
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 40,
            bitWidth = 8,
            notes = "Switch appears when on (but only appears after emptying it out of the block).",
        },
        {
            key = "target_id",
            label = "Target ID",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Switches ID on when switch pressed.",
        },
        {
            key = "clone_settings",
            label = "Clone Settings",
            bitOffset = 24,
            bitWidth = 4,
            notes = "See nybble 7 of Actor 21",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
