// Hostile Neural Networks is one file on the registry: its blocks, recipes and textures are the content mod in this bundle's data/
// (made by tools/make_content.py from the jar). It is written out once and loaded as a dev mod is.
fn hostilenetworks_embedded_content(root: &std::path::Path, enabled: &std::collections::BTreeSet<String>, dev: &[std::path::PathBuf]) -> core::ops::ControlFlow<std::path::PathBuf> {
    static CONTENT: &[u8] = horizon_client::native_data!("hostilenetworks", "hostilenetworks-content.hmod");
    let dir = std::env::current_dir().unwrap_or_default().join(".horizon-cache");
    let file = dir.join("hostilenetworks-embedded-content.hmod");
    if dev.contains(&file) {
        return core::ops::ControlFlow::Continue(());
    }
    let same = std::fs::read(&file).is_ok_and(|on_disk| on_disk == CONTENT);
    if !same && std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, CONTENT)).is_err() {
        log::warn!("Horizon: Hostile Neural Networks' content could not be written to {}", file.display());
        return core::ops::ControlFlow::Continue(());
    }
    let mut with = dev.to_vec();
    with.push(file);
    core::ops::ControlFlow::Break(content_root(root, enabled, &with))
}
