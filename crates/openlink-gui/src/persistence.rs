use std::fs;
use std::path::PathBuf;
use crate::state::SavedStation;

const APP_DIR: &str = "openlink-gui";
const SAVED_STATIONS_FILE: &str = "saved_stations.json";

/// Get the config directory for the application, creating it if needed.
fn config_dir() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join(APP_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir).ok()?;
    }
    Some(dir)
}

/// Load saved stations from the local config file.
pub fn load_saved_stations() -> Vec<SavedStation> {
    let Some(path) = config_dir().map(|d| d.join(SAVED_STATIONS_FILE)) else {
        eprintln!("[persistence] Could not determine config directory");
        return Vec::new();
    };

    if !path.exists() {
        return Vec::new();
    }

    match fs::read_to_string(&path) {
        Ok(content) => {
            match serde_json::from_str::<Vec<SavedStation>>(&content) {
                Ok(stations) => {
                    println!("[persistence] Loaded {} saved station(s) from {}", stations.len(), path.display());
                    stations
                }
                Err(e) => {
                    eprintln!("[persistence] Failed to parse {}: {e}", path.display());
                    Vec::new()
                }
            }
        }
        Err(e) => {
            eprintln!("[persistence] Failed to read {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// Save the list of stations to the local config file.
pub fn save_saved_stations(stations: &[SavedStation]) {
    let Some(path) = config_dir().map(|d| d.join(SAVED_STATIONS_FILE)) else {
        eprintln!("[persistence] Could not determine config directory");
        return;
    };

    match serde_json::to_string_pretty(stations) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, &json) {
                eprintln!("[persistence] Failed to write {}: {e}", path.display());
            } else {
                println!("[persistence] Saved {} station(s) to {}", stations.len(), path.display());
            }
        }
        Err(e) => {
            eprintln!("[persistence] Failed to serialize stations: {e}");
        }
    }
}
