fn kit_guide_dragged_hostilenetworks(delta: [f32; 2], canceled: &mut bool) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    crate::game::hz_kit_guide_hostilenetworks::drag(delta, canceled);
} else { let _ = (delta, canceled); () })
}
