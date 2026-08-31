-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 117,
    name = "Scuttlebug",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A spider that will either drop to the ground or hang on it's web.",
    grabBounds = { x = -14, y = 64, width = 44, height = 37 },
    renderBounds = { x = -14, y = 64, width = 44, height = 37 },
    sourceFiles = {
        "/enemy/gasagoso.nsbmd",
    },
    fields = {
        {
            key = "don_t_wait_for_player",
            label = "Don't wait for player",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Doesn't wait for player to get near to drop/move down. Used by Scuttlebug Spawner.",
        },
        {
            key = "drop_height",
            label = "Drop height",
            bitOffset = 4,
            bitWidth = 4,
            valueOffset = 4,
        },
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1=ignores nybble 10. 2=same as 1 but it falls off screen when stomped instead of getting squished; unused.",
            choices = {
                { value = 0, label = "Hangs on web" },
                { value = 1, label = "Drops on ground" },
                { value = 2, label = "Drops on ground (different stomp)" },
            },
        },
    },
    model = readModel("gasagoso.nsbmd"),
}
