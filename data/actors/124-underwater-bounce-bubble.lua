-- Ported from NSMB Editor 5's NSMBStageObj.cs case 124.

local images = {}

local function init(ctx)
    images.UnderwaterBounceBall = ctx.loadImage("images/UnderwaterBounceBall.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    local BouncePosX = (d[5] & 0xF0) // 8
    if d[5] % 0x2 == 1 then
        BouncePosX = BouncePosX * (-1)
    end
    x = x + BouncePosX
    x = x - 26
    y = y - 26
    width = 53
    height = 53
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    local BounceX = (d[5] & 0xF0) // 8
    if d[5] % 0x2 == 1 then
        BounceX = BounceX * (-1)
    end
    BounceX = BounceX + RenderX
    drawn = gfx.blit(renderer, images.UnderwaterBounceBall, BounceX - 26, RenderY - 26) or drawn
    return drawn
end

return {
    objectId = 124,
    name = "Underwater bounce bubble",
    category = "Enemies - Water",
    notes = "If it is ground pounded it will be destroyed, which is a unused behaviour.",
    fields = {
        {
            key = "shift_position",
            label = "Shift position",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Unused.",
            choices = {
                { value = 0, label = "No shifting" },
                { value = 17, label = "2 pixels left" },
                { value = 33, label = "4 pixels left" },
                { value = 49, label = "6 pixels left" },
                { value = 65, label = "8 pixels left" },
                { value = 81, label = "10 pixels left" },
                { value = 97, label = "12 pixels left" },
                { value = 113, label = "14 pixels left" },
                { value = 129, label = "16 pixels left" },
                { value = 145, label = "18 pixels left" },
                { value = 161, label = "20 pixels left" },
                { value = 177, label = "22 pixels left" },
                { value = 193, label = "24 pixels left" },
                { value = 209, label = "26 pixels left" },
                { value = 225, label = "28 pixels left" },
                { value = 241, label = "30 pixels left" },
                { value = 16, label = "2 pixels right" },
                { value = 32, label = "4 pixels right" },
                { value = 48, label = "6 pixels right" },
                { value = 64, label = "8 pixels right" },
                { value = 80, label = "10 pixels right" },
                { value = 96, label = "12 pixels right" },
                { value = 112, label = "14 pixels right" },
                { value = 128, label = "16 pixels right" },
                { value = 144, label = "18 pixels right" },
                { value = 160, label = "20 pixels right" },
                { value = 176, label = "22 pixels right" },
                { value = 192, label = "24 pixels right" },
                { value = 208, label = "26 pixels right" },
                { value = 224, label = "28 pixels right" },
                { value = 240, label = "30 pixels right" },
            },
        },
        {
            key = "destroyer_id_1",
            label = "Destroyer ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Unused.",
        },
        {
            key = "destroyer_id_2",
            label = "Destroyer ID 2",
            bitOffset = 40,
            bitWidth = 8,
            notes = "Unused.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
