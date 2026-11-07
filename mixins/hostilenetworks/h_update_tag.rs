fn hostilenetworks_update_tag(entity: &horizon_client::containers::ModBlockEntity, tag: &mut Option<horizon_world::nbt::Compound>) {
    crate::game::hostilenetworks::update_tag(entity, tag);
}
