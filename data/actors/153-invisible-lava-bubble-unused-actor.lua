-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 153,
    name = "Invisible Lava Bubble (unused actor)",
    category = "Enemies - Fire",
    notes = "- It is an invisible fireball that moves left and right from its position\n- Phantom Hands will point to the invisible fireball location. It is in the Class Hierachy after the Phantom Hand and it is after the Phantom Hand in the actors and in the sprites. It is also in the same sprite set as the Phantom Hand, and has the same prefix as the Phantom Hand in the Class Hierachy (2291).\n- Phantom Hands ignore Flying ? Blocks they are linked to, when there is an Invisible Fire Bubble in the view.\n- It will show when you get hurt by it. It then pauses and moves a few pixels left and right, before continuing its normal movement.\n- Being invincible or huge does not kill it, but you do not get hurt by it and it does not show when you touch it while invincible/huge.\n- It is invulnerable to the player's fireballs, the shell attack and throwing shells at it.\n- Uses the same graphics as the normal Fire Bubble.\n- It has the collision type that enemies use.",
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
