// `frame`: the layer's work while JEI is on; its window shut when JEI is turned off.
fn kit_layer_frame_hostilenetworks(game: &mut crate::game::Game<'_>) {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    if game.native_on(crate::game::jei::ID) {
        crate::game::hz_kit_layer_hostilenetworks::frame(game);
    } else if crate::game::hz_kit_window_hostilenetworks::is_open() {
        // JEI turned off with its window open: nothing would close it, and it would keep taking the screen's input.
        crate::game::hz_kit_window_hostilenetworks::close();
        let _ = crate::game::hz_kit_window_hostilenetworks::take_acts();
    }
} else { let _ = game; () })
}
