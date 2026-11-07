fn jei_hud_after(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    crate::game::hz_kit_window_hostilenetworks::hud_after(game, batch);
} else { () })
}
