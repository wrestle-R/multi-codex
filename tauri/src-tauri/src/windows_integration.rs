use serde::Serialize;
#[derive(Clone, Debug)]
pub struct DesktopIntegration;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopIntegrationStatus {
    pub available: bool,
    pub installed: bool,
    pub desktop_shortcut: bool,
    pub version: String,
    pub source: String,
}
impl DesktopIntegration {
    pub fn discover() -> Result<Self, String> {
        Ok(Self)
    }
    pub fn status(&self) -> DesktopIntegrationStatus {
        DesktopIntegrationStatus {
            available: false,
            installed: true,
            desktop_shortcut: false,
            version: env!("CARGO_PKG_VERSION").into(),
            source: "package".into(),
        }
    }
    pub fn install(&self, _: bool) -> Result<DesktopIntegrationStatus, String> {
        Ok(self.status())
    }
}
