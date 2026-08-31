-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 59,
    name = "Lakitu spawner",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Spawns Lakitu (40).\n\nThe lakitu spawns if Mario is inside Lakitu inside the Zone. If Lakitu is killed, it is respawned. If Mario exits the Zone, Lakitu despawns.\nIf Sprite Set 5:1 (Bullet Bill/Bob-omb) isn't enabled, the game will freeze when Lakitu tries to display the Spiny egg model.\n",
    grabBounds = { x = 0, y = -23, width = 26, height = 39 },
    renderBounds = { x = 0, y = -23, width = 26, height = 39 },
    sourceFiles = {
        "/enemy/jugem_body.nsmbd",
        "/enemy/jugem_cloud.nsmbd",
    },
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "spiny_type",
            label = "Spiny type",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Rolling egg" },
            },
        },
        {
            key = "z_order",
            label = "Z Order",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Changes Z order so Spiny appear behind Lakitu when thrown. Unused.",
            checkboxValue = 1,
        },
    },
}
