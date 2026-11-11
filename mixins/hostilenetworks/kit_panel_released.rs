fn jei_window_released(screen: &crate::game::native_hooks::NativeScreenView<'_>) {
    hostilenetworks_optional::if_kit_jei!("hostilenetworks", {
    crate::game::hz_kit_window_hostilenetworks::release(screen.mouse);
} else { let _ = screen; () })
}
