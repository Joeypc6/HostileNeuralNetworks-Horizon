fn kit_guide_pressed_hostilenetworks(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    if game.player.is_none() {
        return;
    }
    let _ = button;
    crate::game::hz_kit_guide_hostilenetworks::click(game, screen.mouse, canceled);
} else { let _ = (game, screen, button, canceled); () })
}
