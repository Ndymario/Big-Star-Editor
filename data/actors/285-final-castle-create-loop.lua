-- Ported from NSMB Editor 5's NSMBStageObj.cs case 285.

local function render(actor, renderer)
    local encodedLength = actor.fields.length
    local pathWidth = math.max(0, ((encodedLength % 16) * 16 + encodedLength // 16 - 15) * 16)
    if pathWidth == 0 then return false end

    local x, y = actor.pixelX, actor.pixelY
    renderer:fillRect({ x = x, y = y + 6, width = pathWidth, height = 4, color = "#000000" })
    renderer:fillRect({ x = x + pathWidth - 4, y = y, width = 4, height = 10, color = "#000000" })
    renderer:fillRect({ x = x + 1, y = y + 7, width = pathWidth - 2, height = 2, color = "#ffffff" })
    renderer:fillRect({ x = x + pathWidth - 3, y = y + 1, width = 2, height = 8, color = "#ffffff" })
    return false
end

return {
    objectId = 285,
    name = "Final Castle Create Loop",
    category = "Other",
    fields = {
        {
            key = "id",
            label = "ID",
            bitOffset = 0,
            bitWidth = 8,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Actual width is found by reversing the nybbles and subtracting 15",
        },
    },
    render = render,
}
