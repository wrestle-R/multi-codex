/// Keep the native downloader on the exact artifact emitted by updater-manifest.mjs.
/// The Tauri updater independently verifies the payload with the bundled public key.
pub(crate) fn matches_release_asset(os: &str, arch: &str, version: &str, url: &str) -> bool {
    let suffix = match (os, arch) {
        ("linux", "x86_64") => "amd64.AppImage",
        ("macos", "aarch64") => "aarch64.app.tar.gz",
        ("windows", "x86_64") => "x64-setup.exe",
        _ => return false,
    };
    url == format!(
        "https://github.com/wrestle-R/multi-codex/releases/download/v{version}/Multi.Codex_{version}_{suffix}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_expected_release_and_platform_artifact_is_accepted() {
        let base = "https://github.com/wrestle-R/multi-codex/releases/download";
        for (os, arch, suffix) in [
            ("linux", "x86_64", "amd64.AppImage"),
            ("macos", "aarch64", "aarch64.app.tar.gz"),
            ("windows", "x86_64", "x64-setup.exe"),
        ] {
            let package = format!("{base}/v1.2.3/Multi.Codex_1.2.3_{suffix}");
            assert!(matches_release_asset(os, arch, "1.2.3", &package));
            assert!(!matches_release_asset(os, arch, "1.2.4", &package));
            assert!(!matches_release_asset(os, "other", "1.2.3", &package));
            assert!(!matches_release_asset(
                os,
                arch,
                "1.2.3",
                &format!("{package}?download=1")
            ));
            assert!(!matches_release_asset(
                os,
                arch,
                "1.2.3",
                &package.replace("github.com", "example.invalid"),
            ));
        }
    }
}
