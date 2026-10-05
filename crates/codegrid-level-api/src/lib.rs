//! Versioned host-neutral level API and complete privacy-safe JSON responses.
mod profile;
mod profile_v2;
mod projection_v2;
pub use projection_v2::project_scene_result;
mod session;
mod session_v2;
pub use codegrid_model::{
    error_message, error_number, fallback_error_message, normalize_error_locale, ERROR_LOCALES,
};
pub use profile::{ApiError, SafetyProfile};
pub use profile_v2::{SafetyProfileV2, SceneProfileLimits};
pub use session::LevelApi;
pub use session_v2::LevelApiV2;
pub const LEVEL_API_VERSION: u32 = 1;
