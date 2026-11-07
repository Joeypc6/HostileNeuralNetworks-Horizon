fn kit_guide_scrolled_hostilenetworks(game: &crate::game::Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    if game.player.is_some() {
        crate::game::hz_kit_guide_hostilenetworks::scroll(scroll[1], canceled);
    }
} else { () })
}
