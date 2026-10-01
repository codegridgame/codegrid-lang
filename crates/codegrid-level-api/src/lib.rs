//! Versioned host-neutral level API and complete privacy-safe JSON responses.
mod profile;
mod session;
pub use codegrid_model::{
    error_message, error_number, fallback_error_message, normalize_error_locale, ERROR_LOCALES,
};
pub use profile::{ApiError, SafetyProfile};
pub use session::LevelApi;
pub const LEVEL_API_VERSION: u32 = 1;
