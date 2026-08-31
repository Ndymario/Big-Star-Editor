-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 276,
    name = "Auto-scrolling controller",
    category = "Camera controllers",
    color = "#3f9ca8",
    shape = "box",
    known = true,
    complete = false,
    notes = "==Old==\nThis sprite has to use together with paths. Unknown 1 is for speed. It should be divisible by 8. Unknown 3 maybe acceleration from former node's speed to this node's speed. \n\nAnd I've found some interesting formula: x=98k/y. Where k is the number of pixels being passed within x Mario'\\s second. And y is the value you entered in unknown 1. It relates together. But it might not always work. It may have something to do with unknown 3 too...\n\nCalculation example: let say you want a sprite to move camera up 4 tiles within 16 Mario seconds. Your formula is x=98k/y. As 4 tiles=64 pixels. Substitute it, then you have to multiply it with 98. 98x64=6272. And substitute x with 16. Then 16=6272/y.\nAnd solve it to find y. After you got y, you insert that number into unknown 1. And test it in the emu, can 4 tiles be passed within 16 Mario seconds or not? But I think it does. Remember unknown 3? 0. Sometimes the time doesn't have to be a whole number. \nAlso, I think the position where the sprite is first recognized by the game will determine the camera movement along the path. (Camera movement to the right, left, or straightly of the path) And there're some nybbles controlling how the sprite will act; wait for Mario to come near then scroll or scroll immediately when recognized. I don't know which, but it does have.\nThat's all I've found. :)\nHope this help!\n\nThis sprite does not work properly with Sprite 84: Zoom. It may result in uneven speed, even with the speed for all nodes set equal.",
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "path_id",
            label = "Path ID",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Sets the path the camera will follow while scrolling. Unknown 1 of the first node is target speed and and unknown 3 is something to do with time.",
        },
        {
            key = "loads_from",
            label = "Loads from?",
            bitOffset = 16,
            bitWidth = 4,
            notes = "1 is unused.",
            choices = {
                { value = 0, label = "Midway Point" },
                { value = 1, label = "Unknown 1" },
                { value = 2, label = "Level Start" },
            },
        },
        {
            key = "scrolling_direction",
            label = "Scrolling direction",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Vertical" },
                { value = 1, label = "Horizontal" },
            },
        },
        {
            key = "starting_node",
            label = "Starting node",
            bitOffset = 24,
            bitWidth = 8,
        },
        {
            key = "start_from_both_midway_and_level_start",
            label = "Start from both Midway and Level Start",
            bitOffset = 20,
            bitWidth = 4,
            checkboxValue = 8,
        },
    },
}
