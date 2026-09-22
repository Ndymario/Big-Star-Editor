-- Ported from NSMB Editor 5's NSMBStageObj.cs case 87.

local function grabBounds(actor)
    return {
        x = actor.pixelX,
        y = actor.pixelY,
        width = 16 * actor.fields.length,
        height = 16 * actor.fields.height,
    }
end

local function render(actor, renderer)
    local rect = grabBounds(actor)
    rect.x, rect.y = rect.x + 1, rect.y + 1
    local drawn = renderer:strokeRect(gfx.colored(rect, "#000000"))
    rect.x, rect.y = rect.x - 1, rect.y - 1
    drawn = renderer:strokeRect(gfx.colored(rect, "#00ff00")) or drawn
    renderer:drawText("Warp", { x = actor.pixelX, y = actor.pixelY })
    return drawn
end

return {
    objectId = 87,
    name = "Special exit controller (warp entrance)",
    category = "Event controllers",
    notes = "AKA:\nReggie! Name = Special Exit Controller\nNSMB2 internal actor name = NextGotoBlock\nThe actor will apparently delete itself after use and it will only re-spawn if you go to another area and back. Could be worked around by using multiple warps. Untested if event activation can undelete the actor.\n\nTarget event: use this ID for the warp to activate with the ID, and then deactivate after being used.",
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
