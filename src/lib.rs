pub mod config;
pub mod manager;
pub mod models;
mod utils;

// Re-export the public-facing components from the new modules
// to maintain the same public API for the library's users.
pub use config::ZeroConfig;
pub use manager::ServiceManager;
pub use models::ClientNotification;
