fn hostilenetworks_net_from_client(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::hostilenetworks::learner_message(game, player, channel, data);
}
