//! Typed app and catalog projections. Only core's format module decodes stored JSON.
use hitslop_core::app::{
    self, AppMetadata, Author, Background, Category, StandardFrame, ThemeInput, Views, WindowDefinition, WindowFrame,
};
use hitslop_core::file::{Kind, ResourceInfo, ResourceRoute, Summary};
use hitslop_core::shape::Silhouette;

#[uniffi::remote(Enum)]
pub enum Category {
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
    pub shape: Silhouette,
}
#[uniffi::remote(Record)]
pub struct ThemeInput {
    pub token: String,
    pub color: String,
}
#[uniffi::remote(Record)]
pub struct Views {
    pub export: bool,
    pub icon: bool,
}
#[uniffi::remote(Record)]
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

/// The native view of an accepted app. The original descriptor is opaque to Swift.
#[derive(uniffi::Record)]
pub struct AppDefinition {
    pub metadata: AppMetadata,
    pub window: WindowDefinition,
    pub theme: Vec<ThemeInput>,
    pub views: Views,
    pub descriptor_json: String,
}
impl From<&app::AppDefinition> for AppDefinition {
    fn from(app: &app::AppDefinition) -> Self {
        Self {
            metadata: app.metadata().clone(),
            window: app.window().clone(),
            theme: app.theme().to_vec(),
            views: app.views(),
            descriptor_json: app.document_json().into(),
        }
    }
}
#[uniffi::export]
pub fn categories() -> Vec<Category> {
    Category::ALL.to_vec()
}
#[uniffi::export]
pub fn category_name(category: Category) -> String {
    category.name()
}
#[uniffi::export]
pub fn parse_category(name: String) -> Option<Category> {
    Category::parse(name).ok()
}

use hitslop_core::HostLimits;
#[uniffi::remote(Record)]
pub struct HostLimits {
    pub error_text: u64,
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
    pub helper_protocol: u64,
}
#[uniffi::export]
pub fn host_limits() -> HostLimits {
    hitslop_core::host_limits()
}
#[uniffi::export]
pub fn app_resource_policy() -> String {
    hitslop_core::NATIVE_RESOURCE_POLICY.into()
}
