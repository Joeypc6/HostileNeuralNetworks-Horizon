fn kit_guide_shown_hostilenetworks(stack: &ItemStack) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    crate::game::hz_kit_guide_hostilenetworks::hovered(&stack.item);
} else { () })
}
