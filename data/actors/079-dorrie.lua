-- Ported from NSMB Editor 5's NSMBStageObj.cs case 79.

local images = {}

local function init(ctx)
    images.Dorrie = ctx.loadImage("images/Dorrie.png")
    images.DorrieAway = ctx.loadImage("images/DorrieAway.png")
    images.DorrieAwayBig = ctx.loadImage("images/DorrieAwayBig.png")
    images.DorrieAwayBigger = ctx.loadImage("images/DorrieAwayBigger.png")
    images.DorrieAwayBiggest = ctx.loadImage("images/DorrieAwayBiggest.png")
    images.DorrieBig = ctx.loadImage("images/DorrieBig.png")
    images.DorrieBigger = ctx.loadImage("images/DorrieBigger.png")
    images.DorrieBiggest = ctx.loadImage("images/DorrieBiggest.png")
    images.DorrieTowards = ctx.loadImage("images/DorrieTowards.png")
    images.DorrieTowardsBig = ctx.loadImage("images/DorrieTowardsBig.png")
    images.DorrieTowardsBigger = ctx.loadImage("images/DorrieTowardsBigger.png")
    images.DorrieTowardsBiggest = ctx.loadImage("images/DorrieTowardsBiggest.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    local DorrieShiftX1 = 40
    local DorrieShiftX2 = 40
    local DorrieShiftX3 = 51
    local DorrieShiftY1 = 59
    local DorrieShiftY2 = 59
    local DorrieShiftY3 = 59
    local DorrieWidth1 = 82
    local DorrieWidth2 = 83
    local DorrieWidth3 = 115
    local DorrieHeight1 = 75
    local DorrieHeight2 = 75
    local DorrieHeight3 = 76
    if (d[3] & 0x30) // 0x10 == 1 then
        DorrieShiftX1 = 64
        DorrieShiftX2 = 66
        DorrieShiftX3 = 86
        DorrieShiftY1 = 81
        DorrieShiftY2 = 81
        DorrieShiftY3 = 81
        DorrieWidth1 = 131
        DorrieWidth2 = 135
        DorrieWidth3 = 185
        DorrieHeight1 = 120
        DorrieHeight2 = 120
        DorrieHeight3 = 120
    end
    if (d[3] & 0x30) // 0x10 == 2 then
        DorrieShiftX1 = 73
        DorrieShiftX2 = 74
        DorrieShiftX3 = 97
        DorrieShiftY1 = 89
        DorrieShiftY2 = 89
        DorrieShiftY3 = 89
        DorrieWidth1 = 148
        DorrieWidth2 = 151
        DorrieWidth3 = 208
        DorrieHeight1 = 136
        DorrieHeight2 = 135
        DorrieHeight3 = 136
    end
    if (d[3] & 0x30) // 0x10 == 3 then
        DorrieShiftX1 = 98
        DorrieShiftX2 = 99
        DorrieShiftX3 = 132
        DorrieShiftY1 = 112
        DorrieShiftY2 = 112
        DorrieShiftY3 = 111
        DorrieWidth1 = 198
        DorrieWidth2 = 202
        DorrieWidth3 = 277
        DorrieHeight1 = 181
        DorrieHeight2 = 181
        DorrieHeight3 = 181
    end
    if actor.fields.direction == 2 then
        x = x - DorrieShiftX1
        y = y - DorrieShiftY1
        width = DorrieWidth1
        height = DorrieHeight1
    elseif actor.fields.direction == 3 then
        x = x - DorrieShiftX2
        y = y - DorrieShiftY2
        width = DorrieWidth2
        height = DorrieHeight2
    else
        x = x - DorrieShiftX3
        y = y - DorrieShiftY3
        width = DorrieWidth3
        height = DorrieHeight3
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    local Dorrie1 = images.Dorrie
    local Dorrie2 = images.DorrieAway
    local Dorrie3 = images.DorrieTowards
    local DorrieX1 = 51
    local DorrieX2 = 40
    local DorrieX3 = 40
    local DorrieX4 = 51
    local DorrieY1 = 59
    local DorrieY2 = 59
    local DorrieY3 = 59
    local DorrieY4 = 59
    if (d[3] & 0x30) // 0x10 == 1 then
        Dorrie1 = images.DorrieBig
        Dorrie2 = images.DorrieAwayBig
        Dorrie3 = images.DorrieTowardsBig
        DorrieX1 = 86
        DorrieX2 = 64
        DorrieX3 = 66
        DorrieX4 = 86
        DorrieY1 = 81
        DorrieY2 = 81
        DorrieY3 = 81
        DorrieY4 = 81
    end
    if (d[3] & 0x30) // 0x10 == 2 then
        Dorrie1 = images.DorrieBigger
        Dorrie2 = images.DorrieAwayBigger
        Dorrie3 = images.DorrieTowardsBigger
        DorrieX1 = 97
        DorrieX2 = 73
        DorrieX3 = 74
        DorrieX4 = 97
        DorrieY1 = 89
        DorrieY2 = 89
        DorrieY3 = 89
        DorrieY4 = 89
    end
    if (d[3] & 0x30) // 0x10 == 3 then
        Dorrie1 = images.DorrieBiggest
        Dorrie2 = images.DorrieAwayBiggest
        Dorrie3 = images.DorrieTowardsBiggest
        DorrieX1 = 132
        DorrieX2 = 98
        DorrieX3 = 99
        DorrieX4 = 132
        DorrieY1 = 111
        DorrieY2 = 112
        DorrieY3 = 112
        DorrieY4 = 111
    end
    if actor.fields.direction == 1 then
        Dorrie1 = gfx.transform(Dorrie1, 0, true, false)
        drawn = gfx.blit(renderer, Dorrie1, RenderX - DorrieX1, RenderY - DorrieY1) or drawn
    elseif actor.fields.direction == 2 then
        drawn = gfx.blit(renderer, Dorrie2, RenderX - DorrieX2, RenderY - DorrieY2) or drawn
    elseif actor.fields.direction == 3 then
        drawn = gfx.blit(renderer, Dorrie3, RenderX - DorrieX3, RenderY - DorrieY3) or drawn
    else
        drawn = gfx.blit(renderer, Dorrie1, RenderX - DorrieX4, RenderY - DorrieY4) or drawn
    end
    return drawn
end

return {
    objectId = 79,
    name = "Dorrie",
    category = "Platforms and ropes",
    notes = "If end path node is not set, Dorrie will keep going back and forth between last two nodes. Set end as value 1 in unknown 6.\n\nUsing value 256 in unknown 3 will lower Dorrie's head. 512 will raise it again. Set this at each node you want a change or it will continue from last node setting.",
    fields = {
        {
            key = "path_id",
            label = "Path ID",
            bitOffset = 28,
            bitWidth = 4,
        },
        {
            key = "starting_node",
            label = "Starting node",
            bitOffset = 0,
            bitWidth = 8,
            notes = "255=Appear Always, 0=Appear at level start, Anything else=Appear from midway",
        },
        {
            key = "size",
            label = "Size",
            bitOffset = 20,
            bitWidth = 4,
            notes = "Size is 1+(N6 mod 4).",
            valueOffset = 1,
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 24,
            bitWidth = 4,
            notes = "2 and 3 don't function properly. 1, 2 and 3 are Unused.",
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
                { value = 2, label = "Towards background" },
                { value = 3, label = "Away from background" },
            },
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Starts facing away from background and turns to actual direction when activated. If set to zero, Dorrie won't start until touched by Mario. Unused.",
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
            notes = "Starts facing away from background and turns to actual direction when activated. If set to zero, Dorrie won't start until touched by Mario. Unused.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
