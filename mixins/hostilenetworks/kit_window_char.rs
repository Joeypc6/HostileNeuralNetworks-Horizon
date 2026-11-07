fn jei_window_char(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    if game.player.is_none() || *canceled {
        return;
    }
    if crate::game::hz_kit_window_hostilenetworks::is_open() && !c.is_control() {
        // Typed into the panel's search box when it has the focus; the game's own hook hands it over.
    }
    crate::game::hz_kit_creative_hostilenetworks::char_typed(c, canceled);
} else { () })
}
