-- Ported from NSMB Editor 5's NSMBStageObj.cs case 151.

local images = {}

local function init(ctx)
    images.RedCoinRing = ctx.loadImage("images/RedCoinRing.png")
end

local function grabBounds(actor)
    local x = actor.pixelX - 7
    if actor.fields.shift_8_pixels_right then
        x = x + 8
    end
    return { x = x, y = actor.pixelY - 26, width = 30, height = 42 }
end

local function render(actor, renderer)
    return renderer:draw(images.RedCoinRing, grabBounds(actor))
end

return {
    objectId = 151,
    name = "Red Coin Ring",
    category = "Event controllers",
    notes = "A.k.a. “Red Coin Ring”.\nNote: \"obj_e3_ring.nsbmd\" cannot be opened with NSMBe, to fix this problem, see this tutorial:\n\nhttp://nsmbhd.net/thread/3250-fixing-already-opened-files-tutorial/",
    fields = {
        {
            key = "target_id",
            label = "Target ID",
            bitOffset = 32,
            bitWidth = 8,
            notes = "It can trigger anything, but it's usually used to trigger Red Coins.",
        },
        {
            key = "disable_timer",
            label = "Disable timer",
            bitOffset = 20,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "shift_8_pixels_right",
            label = "Shift 8 pixels right",
            bitOffset = 4,
            bitWidth = 4,
            notes = "If set, the Red Ring will respawn if you change areas and come back.",
            checkboxValue = 1,
        },
        {
            key = "switch_tile_states",
            label = "Switch Tile States",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Switches the state of all two-state tiles; invisible blue coins appear, red coin outlines become collectable, red switch blocks become solid.",
            choices = {
                { value = 0, label = "No" },
                { value = 1, label = "Like a P Switch" },
                { value = 2, label = "Like a ! Switch" },
                { value = 4, label = "Red Blocks only" },
                { value = 5, label = "Like a P Switch + Red Blocks" },
                { value = 7, label = "All switchable tiles" },
            },
        },
        {
            key = "id_activation_mode",
            label = "ID Activation mode",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Works only if Target ID is not equal to 0 and Disable timer is not set.",
            choices = {
                { value = 0, label = "During timer" },
                { value = 2, label = "After timer" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
