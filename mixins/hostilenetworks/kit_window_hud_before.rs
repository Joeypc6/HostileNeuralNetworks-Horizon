fn jei_hud_before(game: &mut crate::game::Game<'_>) {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    crate::game::hz_kit_window_hostilenetworks::hud_before(game);
} else { let _ = game; () })
}
