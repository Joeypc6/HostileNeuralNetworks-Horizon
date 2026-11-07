fn jei_no_recipes_line(text: &str) -> core::ops::ControlFlow<()> {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    if crate::game::hz_kit_layer_hostilenetworks::chat_allowed(text) {
        core::ops::ControlFlow::Continue(())
    } else {
        core::ops::ControlFlow::Break(())
    }
} else { core::ops::ControlFlow::Continue(()) })
}
