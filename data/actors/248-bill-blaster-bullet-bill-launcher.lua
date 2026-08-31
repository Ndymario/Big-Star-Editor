-- Ported from NSMB Editor 5's NSMBStageObj.cs case 248.

local images = {}

local function init(ctx)
    images.BulletBillCannon = ctx.loadImage("images/BulletBillCannon.png")
    images.BulletBillCannonTop = ctx.loadImage("images/BulletBillCannonTop.png")
end

local function grabBounds(actor)
    local height = actor.fields.height
    return {
        x = actor.pixelX,
        y = actor.pixelY - 16 * (height - 1),
        width = 16,
        height = 16 * height,
    }
end

local function render(actor, renderer)
    local drawn = false
    for section = 0, actor.fields.height - 3 do
        drawn = renderer:draw(images.BulletBillCannon, {
            x = actor.pixelX,
            y = actor.pixelY - 16 * section,
        }) or drawn
    end
    drawn = renderer:draw(images.BulletBillCannonTop, {
        x = actor.pixelX,
        y = actor.pixelY - 16 * (actor.fields.height - 1),
    }) or drawn
    return drawn
end

return {
    objectId = 248,
    name = "Bill Blaster (Bullet Bill launcher)",
    category = "Enemies - Castle",
    notes = "Sprite set: 5-1",
    fields = {
        {
            key = "shooting_direction",
            label = "Shooting direction",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 1, label = "Left" },
                { value = 2, label = "Right" },
                { value = 0, label = "Both" },
            },
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 4,
            bitWidth = 4,
            notes = "A height of 9 or more will make the Bill Blaster invisible if the 8 bottommost tiles disappear from view, but still retains collision.",
            valueOffset = 2,
        },
        {
            key = "unknown_8",
            label = "Unknown 8",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Was used by Nintendo devs, somehow as a copy of Nybble 10, but is apparently never called at all, needs testing. Doesn't seem to affect spawned Bullet Bills settings.",
        },
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "is stored by the game for ulterior use but seems to never be called, still needs testing. Doesn't seem to affect spawned Bullet Bills settings.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
