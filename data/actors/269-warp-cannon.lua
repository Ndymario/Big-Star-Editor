-- Ported from NSMB Editor 5's NSMBStageObj.cs case 269.

local images = {}

local function init(ctx)
    images.WarpCannon = ctx.loadImage("images/WarpCannon.png")
    images.WarpWorlds = ctx.loadImage("images/WarpWorlds.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 9
    y = y - 26
    width = 66
    height = 90
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local width, height = 0, 0
    local drawn = false
    drawn = gfx.blit(renderer, images.WarpCannon, RenderX - 9, RenderY - 26) or drawn
    local world = actor.fields.destination_world
    if world >= 5 and world <= 8 then
        world = world - 5
    else
        world = 0
    end
    drawn = gfx.blit(renderer, images.WarpWorlds, RenderX + 26, RenderY + 23, nil, nil, { x = world * 18, y = 0, width = 18, height = 19 }) or drawn
    return drawn
end

return {
    objectId = 269,
    name = "Warp Cannon",
    category = "Bosses and cutscenes",
    notes = "Note: \"w_cannon.nsbmd\" cannot be opened with NSMBe, to fix this problem, see this tutorial:\n\nhttp://nsmbhd.net/thread/3250-fixing-already-opened-files-tutorial/",
    fields = {
        {
            key = "destination_world",
            label = "Destination world",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Worlds 1-4 have no icons. Cannon will appear as \"W5\" for them.",
            choices = {
                { value = 1, label = "World 1" },
                { value = 2, label = "World 2" },
                { value = 3, label = "World 3" },
                { value = 4, label = "World 4" },
                { value = 5, label = "World 5" },
                { value = 6, label = "World 6" },
                { value = 7, label = "World 7" },
                { value = 8, label = "World 8" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
