-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 139,
    name = "Event Controller Random",
    category = "Event controllers",
    notes = "This Event Controller can cause one of the four random IDs to trigger, if a relevant Event ID has been activated. You do not have to use all four Random ID slots, and you can use the same ID on multiple slots. \nExample: \nA in ID1\nA in ID2\nB in ID3. \n2/3 chance of A, 1/3 chance of B.",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
            notes = "This event causes one of the four random IDs to trigger.",
        },
        {
            key = "random_target_id_1",
            label = "Random target ID 1",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "random_target_id_2",
            label = "Random target ID 2",
            bitOffset = 24,
            bitWidth = 8,
        },
        {
            key = "random_target_id_3",
            label = "Random target ID 3",
            bitOffset = 16,
            bitWidth = 8,
        },
        {
            key = "random_target_id_4",
            label = "Random target ID 4",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "usage_mode",
            label = "Usage mode",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Determines, if and how often the relevant Event IDs will be triggered. Events with Values '0' (Single Use) or '1' (Multi Use) will keep their status, unless deactivated/reactivated manually.",
        },
        {
            key = "switch_mode",
            label = "Switch mode",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Activates target" },
                { value = 1, label = "Deactivates target" },
            },
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
