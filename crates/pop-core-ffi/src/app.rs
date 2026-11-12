//! Typed app and catalog projections. Only core's format module decodes stored JSON.
use pop_contracts::app::{
    AppMetadata, Author, Background, Capability, Category, StandardFrame, ThemeInput, Views, WidgetOptions,
    WindowDefinition, WindowFrame,
};
use pop_contracts::resources::{ResourceInfo, ResourceRoute, ResourceSpan, ResponsePlan};
use pop_contracts::shape::Silhouette;
use pop_core::app;
use pop_storage::file::{Kind, Summary};

#[uniffi::remote(Enum)]
#[uniffi(name = "PopCategory")]
pub enum Category {
    Popular,
    Productivity,
    Utilities,
    Finance,
    Media,
    Games,
    DeveloperTools,
    Education,
    Business,
    Personal,
    Health,
    Creative,
    Music,
    Other,
}
#[uniffi::remote(Record)]
pub struct Author {
    pub name: String,
    pub url: Option<String>,
}
#[uniffi::remote(Record)]
pub struct AppMetadata {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: Author,
    pub categories: Vec<Category>,
}
#[uniffi::remote(Enum)]
pub enum Capability {
    Camera,
    Microphone,
    Speech,
}
#[uniffi::remote(Enum)]
pub enum Background {
    Transparent,
    Glass,
}
#[uniffi::remote(Record)]
pub struct WindowDefinition {
    pub width: u32,
    pub height: u32,
    pub fullscreenable: bool,
    pub frame: WindowFrame,
}
#[uniffi::remote(Enum)]
pub enum WindowFrame {
    Standard { frame: StandardFrame },
    Skin { skin: String },
}
#[uniffi::remote(Record)]
pub struct StandardFrame {
    pub fullscreen_fit: bool,
    pub resizable: bool,
    pub lock_aspect: bool,
    pub background: Option<Background>,
    pub radius: String,
    pub shape: Silhouette,
}
#[uniffi::remote(Record)]
#[uniffi(name = "ThemeToken")]
pub struct ThemeInput {
    #[uniffi(name = "name")]
    pub token: String,
    #[uniffi(name = "value")]
    pub color: String,
}
#[uniffi::remote(Record)]
pub struct Views {
    pub preview: bool,
    pub export: bool,
    pub icon: bool,
    pub widget: Option<WidgetOptions>,
}
#[uniffi::remote(Record)]
pub struct WidgetOptions {
    pub refresh_minutes: Option<u32>,
}
#[uniffi::remote(Record)]
#[uniffi(name = "FileSummary")]
pub struct Summary {
    pub kind: Kind,
    pub package_format: u64,
    pub runtime_abi: u64,
    pub metadata: AppMetadata,
    pub bytes: u64,
}
#[uniffi::remote(Enum)]
pub enum ResourceRoute {
    App,
    Attachment,
}
#[uniffi::remote(Record)]
pub struct ResourceInfo {
    pub size: u64,
    pub media_type: String,
}
#[uniffi::remote(Record)]
pub struct ResourceSpan {
    pub offset: u64,
    pub length: u64,
}
#[uniffi::remote(Record)]
pub struct ResponsePlan {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub span: Option<ResourceSpan>,
}
#[uniffi::export]
pub fn resource_response(info: ResourceInfo, route: ResourceRoute, range: Option<String>) -> ResponsePlan {
    pop_contracts::resources::resource_response(&info, route, range.as_deref())
}
/// The native view of an accepted app. The original descriptor is opaque to Swift.
#[derive(uniffi::Record)]
pub struct AppDefinition {
    pub metadata: AppMetadata,
    pub window: WindowDefinition,
    pub theme: Vec<ThemeInput>,
    pub views: Views,
    /// What the app declared that this host implements; the host grants nothing else.
    pub capabilities: Vec<Capability>,
    pub descriptor_json: String,
}
impl From<&app::AppDefinition> for AppDefinition {
    fn from(app: &app::AppDefinition) -> Self {
        Self {
            metadata: app.metadata().clone(),
            window: app.window(),
            theme: app.theme(),
            views: app.views(),
            capabilities: app.capabilities(),
            descriptor_json: app.document_json().into(),
        }
    }
}
use pop_contracts::wire::HostLimits;
#[uniffi::remote(Record)]
pub struct HostLimits {
    pub error_text: u64,
    pub download_bytes: u64,
    pub storage_bytes: u64,
    pub theme_file: u64,
    pub push_items: u64,
    pub push_bytes: u64,
    pub socket_request: u64,
    pub socket_attachment: u64,
    pub image_side: u64,
    pub image_pixels: u64,
    pub min_width: u64,
    pub min_height: u64,
    pub max_window: u64,
    pub package_format: u64,
    pub runtime_abi: u64,
    pub engine_protocol: u64,
}
#[uniffi::export]
pub fn host_limits() -> HostLimits {
    pop_contracts::wire::host_limits()
}
#[uniffi::export]
pub fn app_resource_policy() -> String {
    pop_contracts::policy::NATIVE_RESOURCE_POLICY.into()
}
