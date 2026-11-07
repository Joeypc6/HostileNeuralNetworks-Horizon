fn hostilenetworks_be_energy(block: &str, energy: &mut Option<horizon_client::containers::ModEnergyBuffer>) {
    crate::game::hostilenetworks::block_entity_energy(block, energy);
}
