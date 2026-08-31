-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 58,
    name = "Bullet Bill spawner",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 21, height = 16 },
    renderBounds = { x = 0, y = 0, width = 21, height = 16 },
    sourceFiles = {
        "/enemy/killer.nsmbd",
    },
    fields = {
        {
            key = "shooting_behaviour",
            label = "Shooting behaviour",
            bitOffset = 0,
            bitWidth = 4,
            notes = "2=Fires from sprite vertical placement. Horizontal placement will always be to the right of the screen.",
            choices = {
                { value = 0, label = "Doesn't spawn" },
                { value = 1, label = "Fires at Mario" },
                { value = 2, label = "Fire from sprite placement" },
            },
        },
        {
            key = "maximum_random_up_shift",
            label = "Maximum random up-shift",
            bitOffset = 4,
            bitWidth = 4,
            notes = "0=1. The Bullet Bill, upon spawning, will shift up from a range of pixels between 1 and the value specified.",
        },
        {
            key = "enable_zone_id_check",
            label = "Enable Zone ID check",
            bitOffset = 16,
            bitWidth = 4,
            notes = "It seems like this must be enabled.",
            checkboxValue = 1,
        },
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Spawns from the right edge of screen when player is in zone.",
        },
    },
}
