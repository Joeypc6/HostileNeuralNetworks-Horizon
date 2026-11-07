fn hostilenetworks_interact_entity(game: &mut crate::game::Game<'_>, kind: &str, stack: Option<&ItemStack>, claimed: &mut bool) {
    crate::game::hostilenetworks::interact_entity(game, kind, stack, claimed);
}
