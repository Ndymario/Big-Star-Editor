-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX - 16, y = actor.pixelY, width = 32, height = 16 }
end


return {
    objectId = 230,
    name = "Drawbridge lift",
    category = "Platforms and ropes",
    notes = "Trigger event: unused.\nTarget event: enables/disables movement. Unused.",
    fields = {
        {
            key = "length_of_side",
            label = "Length of side",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used: 9.",
        },
        {
            key = "closed_time",
            label = "Closed time",
            bitOffset = 24,
            bitWidth = 4,
            notes = "Used: 1.",
        },
        {
            key = "open_time",
            label = "Open time",
            bitOffset = 20,
            bitWidth = 4,
            notes = "Used: 1.",
        },
        {
            key = "maximum_angle",
            label = "Maximum angle",
            bitOffset = 16,
            bitWidth = 4,
            notes = "Note : 6 acts like 0. Used: 33.75°.",
            choices = {
                { value = 1, label = "11.25°" },
                { value = 2, label = "22.5°" },
                { value = 3, label = "33.75°" },
                { value = 4, label = "45°" },
                { value = 5, label = "56.25°" },
                { value = 0, label = "67.5°" },
                { value = 7, label = "78.75°" },
                { value = 8, label = "90°" },
                { value = 9, label = "101.25°" },
                { value = 10, label = "112.5°" },
                { value = 11, label = "123.75°" },
                { value = 12, label = "135°" },
                { value = 13, label = "146.25°" },
                { value = 14, label = "157.5°" },
                { value = 15, label = "168.75°" },
            },
        },
        {
            key = "moving",
            label = "Moving",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Used: 3. Unknown how/if >0 values differ.",
            choices = {
                { value = 0, label = "No" },
                { value = 1, label = "Yes" },
                { value = 3, label = "Yes" },
            },
        },
        {
            key = "primary_position",
            label = "Primary position",
            bitOffset = 0,
            bitWidth = 4,
            notes = "For use with the Trigger ID. This is the position the platform assumes when the event is off. The primary position is always up when a Trigger ID is not being used.",
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
            },
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
