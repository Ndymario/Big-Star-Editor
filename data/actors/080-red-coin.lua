-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 80,
    name = "Red Coin",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "When an object with this Event ID is activated (usually a red Coin ring) this Coin will appear.",
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
            notes = "When an object with this Event ID is activated (usually a red Coin ring) this Coin will appear.",
        },
        {
            key = "red_coin_counter_id",
            label = "Red Coin counter ID",
            bitOffset = 0,
            bitWidth = 4,
            notes = "If you have multiple sets of red Coins in a level, each set must have a unique ID or they will not be counted properly.",
        },
    },
}
