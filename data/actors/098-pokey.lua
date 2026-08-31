-- Imported from NSMB Editor 5 stageobjsettings.xml.

local body = loadTexture("sanbo_body.nsbmd")
local head = loadTexture("sanbo_head.nsbmd")

local function pokeyBounds(actor)
    local height = 16 * (2 + (actor.data[5] & 7)) + 9
    return { x = 0, y = 16 - height, width = 18, height = height }
end

local function renderPokey(actor, renderer)
    local y = actor.pixelY
    for _ = 0, actor.data[5] & 7 do
        renderer:draw(body, { x = actor.pixelX, y = y, width = 18, height = 16 })
        y = y - 16
    end
    renderer:draw(head, { x = actor.pixelX, y = y - 9, width = 18, height = 25 })
end

return {
    schema = 1,
    objectId = 98,
    name = "Pokey",
    category = "Enemies - Desert",
    color = "#c9833b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A tall cactus enemy.\n\nUnused behaviours:\n- Can be defeated by Drill Stomp",
    grabBounds = { x = 0, y = -25, width = 18, height = 41 },
    renderBounds = { x = 0, y = -25, width = 18, height = 41 },
    sourceFiles = {
        "/enemy/sanbo_body.nsbmd",
        "/enemy/sanbo_head.nsbmd",
    },
    fields = {
        {
            key = "height",
            label = "Height",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Height formula is 2+(n mod 8) with n being the value of this nybble",
            valueOffset = 2,
        },
    },
    model = readModel("sanbo_head.nsbmd"),
    getGrabBounds = pokeyBounds,
    getRenderBounds = pokeyBounds,
    onRender = renderPokey,
}
