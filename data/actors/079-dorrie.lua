-- Ported from NSMB Editor 5's NSMBStageObj.cs case 79.

local images = {}

local geometry = {
    {
        side = { image = "Dorrie", x = 51, y = 59, width = 115, height = 76 },
        away = { image = "DorrieAway", x = 40, y = 59, width = 82, height = 75 },
        toward = { image = "DorrieTowards", x = 40, y = 59, width = 83, height = 75 },
    },
    {
        side = { image = "DorrieBig", x = 86, y = 81, width = 185, height = 120 },
        away = { image = "DorrieAwayBig", x = 64, y = 81, width = 131, height = 120 },
        toward = { image = "DorrieTowardsBig", x = 66, y = 81, width = 135, height = 120 },
    },
    {
        side = { image = "DorrieBigger", x = 97, y = 89, width = 208, height = 136 },
        away = { image = "DorrieAwayBigger", x = 73, y = 89, width = 148, height = 136 },
        toward = { image = "DorrieTowardsBigger", x = 74, y = 89, width = 151, height = 135 },
    },
    {
        side = { image = "DorrieBiggest", x = 132, y = 111, width = 277, height = 181 },
        away = { image = "DorrieAwayBiggest", x = 98, y = 112, width = 198, height = 181 },
        toward = { image = "DorrieTowardsBiggest", x = 99, y = 112, width = 202, height = 181 },
    },
}

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

local function actorGeometry(actor)
    local size = geometry[actor.fields.size]
    if actor.fields.direction == 2 then return size.away end
    if actor.fields.direction == 3 then return size.toward end
    return size.side
end

local function grabBounds(actor)
    local g = actorGeometry(actor)
    return { x = actor.pixelX - g.x, y = actor.pixelY - g.y, width = g.width, height = g.height }
end

local function render(actor, renderer)
    local g = actorGeometry(actor)
    local bounds = grabBounds(actor)
    bounds.flipX = actor.fields.direction == 1
    return renderer:draw(images[g.image], bounds)
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
            bitWidth = 2,
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
