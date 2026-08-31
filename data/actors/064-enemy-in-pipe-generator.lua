-- Ported from NSMB Editor 5's NSMBStageObj.cs case 64.

local images = {}

local function init(ctx)
    images.BomOmb = ctx.loadImage("images/BomOmb.png")
    images.EnemyGenerator = ctx.loadImage("images/EnemyGenerator.png")
    images.Goomba = ctx.loadImage("images/Goomba.png")
    images.arrow_down = ctx.loadImage("images/arrow_down.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 32, height = 32 }
end

local function render(actor, renderer)
    local enemy = actor.fields.enemy == 0 and images.Goomba or images.BomOmb
    local drawn = renderer:draw(enemy, {
        x = actor.pixelX + 7, y = actor.pixelY + 8, width = 16, height = 18,
    })
    drawn = renderer:draw(images.EnemyGenerator, grabBounds(actor)) or drawn

    local arrow = { x = actor.pixelX + 8, y = actor.pixelY + 32, width = 16, height = 16 }
    local direction = actor.fields.direction % 4
    if direction == 0 then
        arrow.x = actor.pixelX + 8
        arrow.y = actor.pixelY - 16
        arrow.rotation = 180
    elseif direction == 2 then
        arrow.x = actor.pixelX - 16
        arrow.y = actor.pixelY + 8
        arrow.rotation = 90
    elseif direction == 3 then
        arrow.x = actor.pixelX + 32
        arrow.y = actor.pixelY + 8
        arrow.rotation = 270
    else
        arrow.rotation = 0
    end
    return renderer:draw(images.arrow_down, arrow) or drawn
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
