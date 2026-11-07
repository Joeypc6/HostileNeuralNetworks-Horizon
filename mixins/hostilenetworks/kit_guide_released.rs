fn kit_guide_released_hostilenetworks(game: &crate::game::Game<'_>, canceled: &mut bool) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    crate::game::hz_kit_guide_hostilenetworks::release(game, canceled);
} else { () })
}
