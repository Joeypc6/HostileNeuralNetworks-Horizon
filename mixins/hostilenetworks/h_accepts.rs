fn hostilenetworks_accepts(game: &crate::game::Game<'_>, entity: &horizon_client::containers::ModBlockEntity, slot: usize, stack: &horizon_client::items::ItemStack, cancel: &mut bool) {
    crate::game::hostilenetworks::container_accepts(game, entity, slot, stack, cancel);
}
