fn hostilenetworks_destroy(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::hostilenetworks::destroyed(game, pos);
}
