fn kit_guide_render_post_hostilenetworks(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &crate::game::native_hooks::NativeScreenView<'_>) {
    hostilenetworks_optional::if_kit!("hostilenetworks", {
    if game.player.is_none() {
        return;
    }
    crate::game::hz_kit_guide_hostilenetworks::screen_frame(screen.id, screen.menu);
    crate::game::hz_kit_guide_hostilenetworks::draw(game, batch, screen.mouse);
} else { () })
}
