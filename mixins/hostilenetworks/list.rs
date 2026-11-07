// The mod's row in Options > Mods, and its one key.
fn hostilenetworks_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "hostilenetworks".into(),
        title: "Hostile Neural Networks 7.0.0".into(),
        description: "Defeating hostile entities - but in simulations!".into(),
        keys: vec![["guide".to_string(), "Open Guide for Items".to_string(), "key.keyboard.w".to_string()]],
    });
}
