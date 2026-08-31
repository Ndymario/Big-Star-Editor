-- Ported from NSMB Editor 5's NSMBStageObj.cs case 64.

local images = {}

local function init(ctx)
    images.BomOmb = ctx.loadImage("images/BomOmb.png")
    images.EnemyGenerator = ctx.loadImage("images/EnemyGenerator.png")
    images.Goomba = ctx.loadImage("images/Goomba.png")
    images.arrow_down = ctx.loadImage("images/arrow_down.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 32
    height = 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local img, img2
    local drawn = false
    img = images.BomOmb
    if actor.fields.enemy == 0 then
        img = images.Goomba
    end
    drawn = gfx.blit(renderer, img, RenderX + 7, RenderY + 8) or drawn
    drawn = gfx.blit(renderer, images.EnemyGenerator, RenderX, RenderY) or drawn
    img2 = images.arrow_down
    local arrowEnemyGenX = RenderX2
    local arrowEnemyGenY = RenderY2
    if d[5] % 0x4 == 0 then
        arrowEnemyGenX = arrowEnemyGenX + 8
        arrowEnemyGenY = arrowEnemyGenY - 16
        img2 = gfx.transform(img2, 180, false, false)
    elseif d[5] % 0x4 == 2 then
        arrowEnemyGenX = arrowEnemyGenX - 16
        arrowEnemyGenY = arrowEnemyGenY + 8
        img2 = gfx.transform(img2, 90, false, false)
    elseif d[5] % 0x4 == 3 then
        arrowEnemyGenX = arrowEnemyGenX + 32
        arrowEnemyGenY = arrowEnemyGenY + 8
        img2 = gfx.transform(img2, 270, false, false)
    else
        arrowEnemyGenX = arrowEnemyGenX + 8
        arrowEnemyGenY = arrowEnemyGenY + 32
    end
    drawn = gfx.blit(renderer, img2, arrowEnemyGenX, arrowEnemyGenY, 16, 16) or drawn
    return drawn
end

return {
    objectId = 64,
    name = "Enemy-in-Pipe Generator",
    category = "Uncategorized",
    notes = "Uses sound set 8 for Bob-ombs",
    fields = {
        {
            key = "enemy",
            label = "Enemy",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Goomba" },
                { value = 1, label = "Bob-omb" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 2, label = "Left" },
                { value = 3, label = "Right" },
            },
        },
        {
            key = "slower_spawn_rate",
            label = "Slower Spawn Rate",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Unused (From NSMBW)",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
