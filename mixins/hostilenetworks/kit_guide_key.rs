fn kit_guide_key_hostilenetworks(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, key: &str, repeat: bool, canceled: &mut bool) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    if game.player.is_none() {
        return;
    }
    crate::game::hz_kit_guide_hostilenetworks::key(game, screen.menu, key, repeat, canceled);
} else { () })
}
