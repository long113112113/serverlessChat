pub mod client_db;
pub mod database;
pub mod models;
pub mod server_db;

pub use server_db::ServerDatabase;

use std::fs;

/// Ensure data directory exists
pub fn ensure_data_dir() -> std::io::Result<()> {
    fs::create_dir_all("data")?;
    Ok(())
}
