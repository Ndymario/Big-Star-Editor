-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 271,
    name = "Vertical camera offset (unused actor)",
    category = "Camera controllers",
    notes = "(Discovered that it does something: Hiccup)\n(Sprite data/purpose: Sharks)\nIf Mario/Luigi is inside the AOE (Area of Effect) of this actor, the camera will go up by HH pixels, the camera will scroll down back once Mario/Luigi leaves the horizontal boundary specified by the value L.",
    fields = {
        {
            key = "activate_the_sprite_from",
            label = "Activate the sprite from",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Below" },
                { value = 1, label = "Above" },
            },
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 8,
            bitWidth = 4,
            notes = "From 0 to 15. Sprite position is always the leftmost tile.",
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 0,
            bitWidth = 8,
            notes = "From 0 to 255. If Height is set for example at 60, Mario will have to jump 60 pixels above the sprite's AOE for said effect to take place.",
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
