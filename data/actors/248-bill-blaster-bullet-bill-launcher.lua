-- Ported from NSMB Editor 5's NSMBStageObj.cs case 248.

local images = {}

local function init(ctx)
    images.BulletBillCannon = ctx.loadImage("images/BulletBillCannon.png")
    images.BulletBillCannonTop = ctx.loadImage("images/BulletBillCannonTop.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    height = (d[5] & 0xF0) + 32
    y = y - (height - 16)
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    for l = 0, d[5] // 0x10 do
        if l == d[5] // 0x10 then
            drawn = gfx.blit(renderer, images.BulletBillCannonTop, RenderX, RenderY - 16) or drawn
        else
            drawn = gfx.blit(renderer, images.BulletBillCannon, RenderX, RenderY) or drawn
            RenderY = RenderY - 16
        end
    end
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
