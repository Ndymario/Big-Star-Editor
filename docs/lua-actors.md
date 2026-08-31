# Lua actors

Every actor is described by one `.lua` file. Big Star loads actor directories in this order, with the first definition for an ID winning:

1. `<project>/actors`
2. `actors` in each enabled module, in module order
3. `<Big Star config>/actors`
4. the shipped `data/actors` directory

Vanilla actors use their runtime class ID as `objectId`. Module actors use the stable hash written to `build/generated/stageobjects.json` as `hash`. Exactly one of those keys is required.

```lua
return {
    schema = 1,
    objectId = 83,
    name = "Goomba",
    category = "Enemies / Ground",
    color = "#b5651d",
    shape = "ellipse", -- box, ellipse, or diamond

    grabBounds = { x = -8, y = -8, width = 16, height = 16 },
    renderBounds = { x = -8, y = -8, width = 16, height = 16 },

    fields = {
        {
            key = "size",
            label = "Size",
            bitOffset = 0,
            bitWidth = 2,
            choices = {
                { value = 0, label = "Small" },
                { value = 1, label = "Big" },
            },
        },
    },

    graphics = readGraphics("I_kuribo256_ncg.bin"),
    palette = readPalette("I_enemy_ncl.bin"),
}
```

`readGraphics` and `readPalette` resolve a NitroFS file by filename through the active module/variant filesystem view. A 2D actor must provide both. A model actor instead uses `model = readModel("actor.nsbmd")`. Paths and direct string resource values are rejected so a script cannot escape the project resolver.

For a module actor, replace `objectId` with its hash:

```lua
return {
    schema = 1,
    hash = 0x12345678,
    name = "Module Actor",
    model = readModel("module_actor.nsbmd"),
}
```

Actor scripts run without Lua's filesystem, process, package, or standard libraries. Big Star also limits script size, memory, and instruction count. Table literals, local values, expressions, and the three resource functions are available.
