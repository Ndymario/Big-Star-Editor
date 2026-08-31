-- Ported from NSMB Editor 5's NSMBStageObj.cs case 168.

local images = {}

local function init(ctx)
    images.StoneBlockBottom = ctx.loadImage("images/StoneBlockBottom.png")
    images.StoneBlockBottomLeft = ctx.loadImage("images/StoneBlockBottomLeft.png")
    images.StoneBlockBottomRight = ctx.loadImage("images/StoneBlockBottomRight.png")
    images.StoneBlockFlatHorizLeft = ctx.loadImage("images/StoneBlockFlatHorizLeft.png")
    images.StoneBlockFlatHorizMiddle = ctx.loadImage("images/StoneBlockFlatHorizMiddle.png")
    images.StoneBlockFlatHorizRight = ctx.loadImage("images/StoneBlockFlatHorizRight.png")
    images.StoneBlockLeft = ctx.loadImage("images/StoneBlockLeft.png")
    images.StoneBlockMiddle = ctx.loadImage("images/StoneBlockMiddle.png")
    images.StoneBlockRight = ctx.loadImage("images/StoneBlockRight.png")
    images.StoneBlockSpikes = ctx.loadImage("images/StoneBlockSpikes.png")
    images.StoneBlockTop = ctx.loadImage("images/StoneBlockTop.png")
    images.StoneBlockTopLeft = ctx.loadImage("images/StoneBlockTopLeft.png")
    images.StoneBlockTopRight = ctx.loadImage("images/StoneBlockTopRight.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = math.max(16 * (d[5] % 0x10 + 1), 32)
    height = 16 * (d[5] // 0x10 + 1)
    local spikes = actor.fields.spikes
    if width == 32 and (spikes == 1 or spikes == 2) then
        width = 48
    end
    if width == 32 and spikes == 3 then
        width = 64
    end
    if height == 16 and (spikes == 4 or spikes == 5) then
        height = 32
    end
    if height == 16 and spikes == 6 then
        height = 48
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local width, height = 0, 0
    local drawn = false
    width = d[5] % 0x10
    if actor.objectId == 165 then
        width = d[5] % 0x8
    end
    local spikes = 0
    if actor.objectId == 168 then
        height = d[5] // 0x10
        spikes = actor.fields.spikes
        if spikes == 1 or spikes == 3 then
            height = height - 1
            RenderY = RenderY + 16
        end
        if spikes == 2 or spikes == 3 then
            height = height - 1
        end
        if spikes == 4 or spikes == 6 then
            width = width - 1
        end
        if spikes == 5 or spikes == 6 then
            width = width - 1
            RenderX = RenderX + 16
        end
    else
        height = (((2) ^ (((d[4] & 0x30) / 0x10))) - 1)
    end
    if width < 1 then
        width = 1
    end
    if height < 0 then
        height = 0
    end
    local xp = RenderX
    local yp = RenderY
    if height == 0 then
        drawn = gfx.blit(renderer, images.StoneBlockFlatHorizLeft, RenderX, RenderY) or drawn
        for l = 0, (width - 1) - 1 do
            RenderX = RenderX + 16
            drawn = gfx.blit(renderer, images.StoneBlockFlatHorizMiddle, RenderX, RenderY) or drawn
        end
        drawn = gfx.blit(renderer, images.StoneBlockFlatHorizRight, RenderX + 16, RenderY) or drawn
    else
        drawn = gfx.blit(renderer, images.StoneBlockTopLeft, RenderX, RenderY) or drawn
        for l = 0, (width - 1) - 1 do
            RenderX = RenderX + 16
            drawn = gfx.blit(renderer, images.StoneBlockTop, RenderX, RenderY) or drawn
        end
        RenderX = RenderX + 16
        drawn = gfx.blit(renderer, images.StoneBlockTopRight, RenderX, RenderY) or drawn
        for l = 0, (height - 1) - 1 do
            RenderY = RenderY + 16
            drawn = gfx.blit(renderer, images.StoneBlockRight, RenderX, RenderY) or drawn
        end
        RenderY = RenderY + 16
        drawn = gfx.blit(renderer, images.StoneBlockBottomRight, RenderX, RenderY) or drawn
        for l = 0, (width - 1) - 1 do
            RenderX = RenderX - 16
            drawn = gfx.blit(renderer, images.StoneBlockBottom, RenderX, RenderY) or drawn
        end
        RenderX = RenderX - 16
        drawn = gfx.blit(renderer, images.StoneBlockBottomLeft, RenderX, RenderY) or drawn
        for l = 0, (height - 1) - 1 do
            RenderY = RenderY - 16
            drawn = gfx.blit(renderer, images.StoneBlockLeft, RenderX, RenderY) or drawn
        end
        RenderX = xp
        RenderY = yp
        local xStart = RenderX
        for l = 0, (height - 1) - 1 do
            RenderY = RenderY + 16
            for m = 0, (width - 1) - 1 do
                RenderX = RenderX + 16
                drawn = gfx.blit(renderer, images.StoneBlockMiddle, RenderX, RenderY) or drawn
            end
            RenderX = xStart
        end
    end
    if spikes == 1 or spikes == 3 then
        RenderX = xp
        RenderY = yp - 16
        for l = 0, width do
            drawn = gfx.blit(renderer, images.StoneBlockSpikes, RenderX, RenderY) or drawn
            RenderX = RenderX + 16
        end
    end
    if spikes == 2 or spikes == 3 then
        RenderX = xp
        RenderY = ((actor.pixelY / 16) + height + 1) * 16
        if spikes == 3 then
            RenderY = RenderY + 16
        end
        local spikes2 = images.StoneBlockSpikes
        spikes2 = gfx.transform(spikes2, 0, false, true)
        for l = 0, d[5] % 0x10 do
            drawn = gfx.blit(renderer, spikes2, RenderX, RenderY) or drawn
            RenderX = RenderX + 16
        end
    end
    if spikes == 4 or spikes == 6 then
        RenderX = ((actor.pixelX / 16) + width + 1) * 16
        RenderY = yp
        if spikes == 6 then
            RenderX = RenderX + 16
        end
        local spikes2 = images.StoneBlockSpikes
        spikes2 = gfx.transform(spikes2, 90, false, false)
        for l = 0, d[5] // 0x10 do
            drawn = gfx.blit(renderer, spikes2, RenderX, RenderY) or drawn
            RenderY = RenderY + 16
        end
    end
    if spikes == 5 or spikes == 6 then
        RenderX = xp - 16
        RenderY = yp
        local spikes2 = images.StoneBlockSpikes
        spikes2 = gfx.transform(spikes2, 90, true, false)
        for l = 0, height do
            drawn = gfx.blit(renderer, spikes2, RenderX, RenderY) or drawn
            RenderY = RenderY + 16
        end
    end
    return drawn
end

return {
    objectId = 168,
    name = "Stone wall moving left & right or up & down",
    category = "Platforms and ropes",
    fields = {
        {
            key = "horizontal_movement",
            label = "Horizontal movement",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "spikes",
            label = "Spikes",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "None" },
                { value = 1, label = "Top" },
                { value = 2, label = "Bottom" },
                { value = 3, label = "Top and Bottom" },
                { value = 4, label = "Right" },
                { value = 5, label = "Left" },
                { value = 6, label = "Right and left" },
            },
        },
        {
            key = "start_delay",
            label = "Start delay",
            bitOffset = 20,
            bitWidth = 4,
        },
        {
            key = "end_delay",
            label = "End delay",
            bitOffset = 16,
            bitWidth = 4,
        },
        {
            key = "tiles_to_move",
            label = "Tiles to move",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Negative numbers = down/left. Positive numbers up/right. NOT diagonal for example - ticking \"Horizontal movement\" and \"Tiles to move\" to minus -7 will make it move left one block. For vertical movement -8 is no move movement -7 is down one -6 is down two ect",
            signed = true,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Slowest" },
                { value = 1, label = "Medium" },
                { value = 2, label = "Fast" },
                { value = 3, label = "Fastest" },
            },
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 4,
            bitWidth = 4,
            valueOffset = 1,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            valueOffset = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
