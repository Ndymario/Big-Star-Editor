-- Ported from NSMB Editor 5's NSMBStageObj.cs case 87.

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 16 * (d[4] % 0x10 + 1)
    height = 16 * (d[5] // 0x10 + 1)
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    local rect = grabBounds(actor)
    rect.x, rect.y = rect.x + 1, rect.y + 1
    drawn = renderer:strokeRect(gfx.colored(rect, "#000000")) or drawn
    rect.x, rect.y = rect.x + (-1), rect.y + (-1)
    drawn = renderer:strokeRect(gfx.colored(rect, "#00ff00")) or drawn
    renderer:drawText("Warp", { x = RenderX + 1, y = RenderY + 1 })
    renderer:drawText("Warp", { x = RenderX, y = RenderY })
    return drawn
end

return {
    objectId = 87,
    name = "Special exit controller (warp entrance)",
    category = "Event controllers",
    notes = "AKA:\nReggie! Name = Special Exit Controller\nNSMB2 internal actor name = NextGotoBlock\nThe actor will apparently delete itself after use and it will only re-spawn if you go to another area and back. Could be worked around by using multiple warps. Untested if event activation can undelete the actor.",
    fields = {
        {
            key = "warp_mode",
            label = "Warp mode",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Set to this or to 8 if dest. is in another area" },
                { value = 1, label = "If climbing vine" },
                { value = 3, label = "Set to this or to 1 if dest. is in current area" },
                { value = 8, label = "Wavy transition" },
            },
        },
        {
            key = "destination_area",
            label = "Destination area",
            bitOffset = 20,
            bitWidth = 4,
            notes = "Set to 0 when warping to the same area.",
        },
        {
            key = "destination_entrance",
            label = "Destination entrance",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 8,
            bitWidth = 4,
            valueOffset = 1,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 4,
            bitWidth = 4,
            valueOffset = 1,
        },
        {
            key = "allows_mega_mario",
            label = "Allows Mega Mario",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
            notes = "Use this ID fo the warp to activate with the ID, and then deactivate after being used.",
        },
        {
            key = "disable_sound",
            label = "Disable sound",
            bitOffset = 24,
            bitWidth = 4,
            notes = "Disables sound if warping to other area but crashes if destination is in same area.",
            checkboxValue = 1,
        },
    },
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
