fn hostilenetworks_input_closed(game: &mut crate::game::Game<'_>, uuid: u128, items: &mut Vec<Option<ItemStack>>) {
    crate::game::hostilenetworks::menu_input_closed(game, uuid, items);
}
