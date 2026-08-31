-- Ported from NSMB Editor 5's NSMBStageObj.cs case 214.

local images = {}

local function init(ctx)
    images.SnakeBlock = ctx.loadImage("images/SnakeBlock.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16 * actor.fields.length, height = 16 }
end

local function render(actor, renderer)
    for block = 0, actor.fields.length - 1 do
        renderer:draw(images.SnakeBlock, { x = actor.pixelX + 16 * block, y = actor.pixelY })
    end
    return true
end

return {
    objectId = 214,
    name = "Snake Block",
    category = "Platforms and ropes",
    notes = "It destroys any solid tiles it moves over. The sprite uses the green block tile in jyotyu map16. Diagonal directions don't work properly.",
    fields = {
        {
            key = "path_id",
            label = "Path ID",
            bitOffset = 4,
            bitWidth = 4,
            notes = "If you don't have a matching ID, the game will freeze.",
        },
        {
            key = "loads_from",
            label = "Loads from",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Level start" },
                { value = 1, label = "Midway point" },
            },
        },
        {
            key = "starting_node",
            label = "Starting node",
            bitOffset = 16,
            bitWidth = 8,
        },
        {
            key = "last_node_behaviour",
            label = "Last node behaviour",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Game will freeze if N4 mod 8 is 5 or 6. For 3 it skyrockets for 44 blocks and 7 pixels then disappears.",
            choices = {
                { value = 0, label = "Nothing (keeps shape)" },
                { value = 1, label = "Falls apart (doesn't keep shape)" },
                { value = 2, label = "Comes back and forth" },
                { value = 3, label = "Skyrockets (keeps shape)" },
                { value = 4, label = "Falls apart (keeps shape)" },
            },
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 12,
            bitWidth = 4,
            valueOffset = 3,
        },
        {
            key = "slower",
            label = "Slower",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
