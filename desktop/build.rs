// Embeds the app icon in the Windows executable, so Explorer and the Start menu show it.
// Other targets skip it.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("packaging/windows/typo.rc", embed_resource::NONE).manifest_optional().ok();
    }
}
