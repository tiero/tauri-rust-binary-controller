// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::HashMap, os::unix::prelude::PermissionsExt};

use tokio::{
    fs,
    io::AsyncWriteExt,
    sync::Mutex,
    process::{Child, Command},
};


#[derive(Default)]
struct SharedState {
    services: Mutex<HashMap<String, Child>>,
    download_statuses: Mutex<HashMap<String, DownloadStatus>>,
}
// Assuming this is part of your SharedState
struct DownloadStatus {
    is_downloading: bool,
    progress: f32, // percentage of download
}


fn set_execute_permission(path: &std::path::Path) -> Result<(), std::io::Error> {
    let metadata = 
        std::fs::metadata(&path)?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);  // rwxr-xr-x
    std::fs::set_permissions(&path, permissions)
}


// TAURI COMMANDS
#[tauri::command(async)]
async fn download_services(service_ids: Vec<String>, state: tauri::State<'_, SharedState>) -> Result<(), String> {
    let data_dir = tauri::api::path::data_dir().ok_or("Failed to get data dir")?;
    println!("data_dir: {:?}", data_dir);
    let binaries_dir = tauri::api::path::data_dir().map_or_else(
        || Err("Failed to get data dir".to_string()),
        |dir| {
            let path = dir.join("my-app@next").join("binaries");
            std::fs::create_dir_all(&path)
                .map(|_| path)
                .map_err(|e| e.to_string())
        },
    )?;

    println!("binaries_dir: {:?}", binaries_dir);
    for id in &service_ids {
        let filename = binaries_dir.join(&id);

        // Check if file already exists
        println!("Checking if {:?} exists", filename);
        if filename.exists() {
            println!("Skipping download of {:?} because it already exists", filename);
            continue; // Skip download if it exists
        }

        // Check if we're already downloading this file
        println!("Checking if {:?} is already downloading", filename);
        {
            let status_lock = state.download_statuses.lock().await;
            if let Some(status) = status_lock.get(id) {
                if status.is_downloading {
                    continue; // Skip download if already downloading
                }
            }
        }

        // Update status to "downloading"
        println!("Updating {:?} status to downloading", filename);
        {
            let mut status_lock = state.download_statuses.lock().await;
            status_lock.insert(id.clone(), DownloadStatus { is_downloading: true, progress: 0.0 });
        }

        let url = format!("http://localhost:8080/{}.bin", id);
        
        // Download with progress tracking
        println!("Downloading {} from {}", id, url);
        let mut response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
        
        let total_size = response.content_length().ok_or("Failed to get content length")? as usize;
        let mut downloaded = 0usize;

    

        let mut file = tokio::fs::File::create(&filename).await.map_err(|e| e.to_string())?;

        while let Ok(Some(chunk)) = response.chunk().await {
            let chunk = chunk;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            downloaded += chunk.len();

            // Update download progress
            let progress = (downloaded as f32 / total_size as f32) * 100.0;
            {
                let mut status_lock = state.download_statuses.lock().await;
                if let Some(status) = status_lock.get_mut(id) {
                    status.progress = progress;
                }
            }
        }

        // Set execute permission
        println!("Setting execute permission on {:?}", filename);
        set_execute_permission(&filename).map_err(|e| e.to_string())?;
        
        // Mark as done downloading
        println!("Updating {:?} status to not downloading", filename);
        {
            let mut status_lock = state.download_statuses.lock().await;
            if let Some(status) = status_lock.get_mut(id) {
                status.is_downloading = false;
            }
        }
    }
    
    Ok(())
}

// To get download progress for a specific service ID
#[tauri::command(async)]
async fn get_download_progress(service_id: String, state: tauri::State<'_, SharedState>) -> Result<f32, String> {
    let status_lock = state.download_statuses.lock().await;
    if let Some(status) = status_lock.get(&service_id) {
        Ok(status.progress)
    } else {
        Err("No download status for given service ID".to_string())
    }
}#[tauri::command(async)]
async fn run_service(
    service_id: String,
    state: tauri::State<'_, SharedState>,
) -> Result<(), String> {
    let data_dir = tauri::api::path::data_dir().ok_or("Failed to get data dir")?;
    println!("data_dir: {:?}", data_dir);
    let binaries_dir = tauri::api::path::data_dir().map_or_else(
        || Err("Failed to get data dir".to_string()),
        |dir| {
            let path = dir.join("my-app@next").join("binaries");
            std::fs::create_dir_all(&path)
                .map(|_| path)
                .map_err(|e| e.to_string())
        },
    )?;
    let logs_dir = tauri::api::path::data_dir().map_or_else(
        || Err("Failed to get data dir".to_string()),
        |dir| {
            let path = dir.join("my-app@next").join("logs");
            std::fs::create_dir_all(&path)
                .map(|_| path)
                .map_err(|e| e.to_string())
        },
    )?;
    let binary_path = binaries_dir.join(&service_id);
    let log_path = logs_dir.join(format!("{}.log", service_id));
    // Check if binary file exists
    if !binary_path.exists() {
        return Err(format!("Binary for service {} does not exist", service_id));
    }


    
    // Use synchronous std::fs::File for log file creation
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    // Check if service is already running
    let services = state.services.lock().await;
    if services.contains_key(&service_id) {
        return Err(format!("Service {} is already running", service_id));
    }
    drop(services); // Drop the lock before we proceed

    let child = Command::new(binary_path)
        .stdout(std::process::Stdio::from(
            log_file.try_clone().map_err(|e| format!("Failed to clone log file: {}", e))?,
        )) // Redirect stdout to log_file
        .stderr(std::process::Stdio::from(log_file)) // Redirect stderr to the same log_file
        .spawn()
        .map_err(|e| format!("Failed to spawn child process: {}", e))?;

    let mut services = state.services.lock().await;
    services.insert(service_id, child);
    Ok(())
}


#[tauri::command(async)]
async fn stop_service(
    service_id: String,
    state: tauri::State<'_, SharedState>,
) -> Result<(), String> {
    // Lock the async Mutex
    let mut services = state.services.lock().await; 

    if let Some(mut child) = services.remove(&service_id) {
        child.kill().await.map_err(|e| e.to_string())?;
    }

    Ok(())
}
#[tauri::command(async)]
async fn show_logs_for_service(service_id: String) -> Result<String, String> {
    let log_path = dirs::data_dir()
        .ok_or("failed to get app data dir")?
        .join(format!("{}.log", service_id));
    let logs = fs::read_to_string(log_path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(logs)
}
#[tauri::command(async)]
async fn delete_service(service_id: String) -> Result<(), String> {
    let binary_path = dirs::data_dir()
        .ok_or("failed to get app data dir")?
        .join(&service_id);
    fs::remove_file(binary_path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// DEMO COMMANDS, YOU CAN REMOVE THEM
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn main() {
    let state = SharedState::default();
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            download_services,
            get_download_progress,
            run_service,
            stop_service,
            show_logs_for_service,
            delete_service,
            // demo commands, you can remove them
            greet
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
