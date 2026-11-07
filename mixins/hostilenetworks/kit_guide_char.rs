fn kit_guide_char_hostilenetworks(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    let _ = c;
    if game.player.is_some() && crate::game::hz_kit_guide_hostilenetworks::is_open() {
        *canceled = true;
    }
} else { () })
}
