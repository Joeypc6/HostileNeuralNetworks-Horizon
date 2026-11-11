fn kit_guide_tooltip_hostilenetworks(stack: &horizon_client::items::ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    crate::game::hz_kit_guide_hostilenetworks::tooltip(stack, lines);
} else { let _ = (stack, lines); () })
}
