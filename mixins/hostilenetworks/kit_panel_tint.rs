fn jei_panel_tint(game: &Game<'_>, batch: &mut HudBatch) {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    crate::game::hz_kit_window_hostilenetworks::tint(game, batch);
} else { let _ = (game, batch); () })
}
