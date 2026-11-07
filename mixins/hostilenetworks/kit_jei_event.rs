fn ae2_jei_event(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    event: &crate::game::native_hooks::NativePanelEvent,
) {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    crate::game::hz_kit_layer_hostilenetworks::event(game, mod_id, event);
} else { () })
}
