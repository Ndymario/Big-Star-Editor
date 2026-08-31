-- Ported from NSMB Editor 5's NSMBStageObj.cs case 264.

local images = {}

local function init(ctx)
    images.QBlock = ctx.loadImage("images/QBlock.png")
    images.ToadBackground1 = ctx.loadImage("images/ToadBackground1.png")
    images.ToadBackground2 = ctx.loadImage("images/ToadBackground2.png")
    images.ToadBackground3 = ctx.loadImage("images/ToadBackground3.png")
    images.ToadBackground4 = ctx.loadImage("images/ToadBackground4.png")
    images.ToadBackground5 = ctx.loadImage("images/ToadBackground5.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if (d[2] & 0x10) // 0x10 == 1 then
        x = x - 12
        y = y - 12
        width = 40
        height = 40
    end
    if (d[5] & 0x10) // 0x10 == 1 then
        x = x + 8
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.QBlock
    if (d[2] & 0x10) // 0x10 == 1 then
        RenderX = RenderX - 12
        RenderY = RenderY - 12
        if actor.fields.block_id == 1 then
            img = images.ToadBackground2
        elseif actor.fields.block_id == 2 then
            img = images.ToadBackground3
        elseif actor.fields.block_id == 3 then
            img = images.ToadBackground4
        elseif actor.fields.block_id == 4 then
            img = images.ToadBackground5
        else
            img = images.ToadBackground1
        end
    end
    if (d[5] & 0x10) // 0x10 == 1 then
        RenderX = RenderX + 8
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 264,
    name = "Toad House block",
    category = "Bosses and cutscenes",
    fields = {
        {
            key = "bonus_game",
            label = "Bonus game",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 0, label = "1-Up Minigame" },
                { value = 1, label = "Background Chooser" },
            },
        },
        {
            key = "unknown_7",
            label = "Unknown 7",
            bitOffset = 16,
            bitWidth = 4,
            notes = "1 is used when big roulette block is spawned by Toadsworth",
        },
        {
            key = "shift_position",
            label = "Shift position",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "No shifting" },
                { value = 1, label = "8 pixels right" },
            },
        },
        {
            key = "block_id",
            label = "Block ID",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "1" },
                { value = 1, label = "2" },
                { value = 2, label = "3" },
                { value = 3, label = "4" },
                { value = 4, label = "5" },
                { value = 5, label = "6" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
