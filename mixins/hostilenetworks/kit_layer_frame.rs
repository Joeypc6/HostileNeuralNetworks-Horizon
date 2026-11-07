fn kit_layer_frame_hostilenetworks(game: &mut crate::game::Game<'_>) {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    if game.native_on(crate::game::jei::ID) {
        crate::game::hz_kit_layer_hostilenetworks::frame(game);
    }
} else { () })
}
