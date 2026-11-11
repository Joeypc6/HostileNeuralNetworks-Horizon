fn kit_guide_frame_hostilenetworks(game: &mut crate::game::Game<'_>) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    if game.player.is_some() {
        crate::game::hz_kit_guide_hostilenetworks::frame_hook(game);
    }
} else { let _ = game; () })
}
