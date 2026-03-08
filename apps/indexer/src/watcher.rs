use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use sqlx::Row;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, trace};
use uuid::Uuid;

use crate::utils;
use crate::AppState;

pub async fn run_file_watcher(state: AppState) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<(Uuid, String)>(100);

    // Start watcher refresh loop
    let refresh_interval = Duration::from_secs(30);
    let pool = state.pool.clone();

    tokio::spawn(async move {
        let mut watched_libraries: HashMap<Uuid, String> = HashMap::new();

        loop {
            // Get libraries with watcher enabled
            match sqlx::query(
                "SELECT id, root_path FROM libraries WHERE watcher_enabled = TRUE AND enabled = TRUE"
            )
            .fetch_all(&pool)
            .await
            {
                Ok(rows) => {
                    let current_libraries: HashMap<Uuid, String> = rows
                        .into_iter()
                        .map(|row| {
                            let id: Uuid = row.get("id");
                            let root_path: String = row.get("root_path");
                            let local_path = utils::remap_libraries_path(&root_path);
                            (id, local_path)
                        })
                        .collect();

                    // Check if we need to recreate watcher
                    let needs_restart = watched_libraries.len() != current_libraries.len()
                        || watched_libraries.iter().any(|(id, path)| {
                            current_libraries.get(id) != Some(path)
                        });

                    if needs_restart {
                        info!("[WATCHER] Restarting watcher for {} libraries", current_libraries.len());

                        if !current_libraries.is_empty() {
                            let tx_clone = tx.clone();
                            let libraries_clone = current_libraries.clone();

                            match setup_watcher(libraries_clone, tx_clone) {
                                Ok(_new_watcher) => {
                                    watched_libraries = current_libraries;
                                    info!("[WATCHER] Watching {} libraries", watched_libraries.len());
                                }
                                Err(err) => {
                                    error!("[WATCHER] Failed to setup watcher: {}", err);
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    error!("[WATCHER] Failed to fetch libraries: {}", err);
                }
            }

            tokio::time::sleep(refresh_interval).await;
        }
    });

    // Process watcher events
    while let Some((library_id, file_path)) = rx.recv().await {
        info!("[WATCHER] File changed in library {}: {}", library_id, file_path);

        // Check if there's already a pending job for this library
        match sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id = $1 AND status IN ('pending', 'running'))"
        )
        .bind(library_id)
        .fetch_one(&state.pool)
        .await
        {
            Ok(exists) => {
                if !exists {
                    // Create a quick scan job
                    let job_id = Uuid::new_v4();
                    match sqlx::query(
                        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'rebuild', 'pending')"
                    )
                    .bind(job_id)
                    .bind(library_id)
                    .execute(&state.pool)
                    .await
                    {
                        Ok(_) => info!("[WATCHER] Created job {} for library {}", job_id, library_id),
                        Err(err) => error!("[WATCHER] Failed to create job: {}", err),
                    }
                } else {
                    trace!("[WATCHER] Job already pending for library {}, skipping", library_id);
                }
            }
            Err(err) => error!("[WATCHER] Failed to check existing jobs: {}", err),
        }
    }

    Ok(())
}

fn setup_watcher(
    libraries: HashMap<Uuid, String>,
    tx: mpsc::Sender<(Uuid, String)>,
) -> Result<RecommendedWatcher> {
    let libraries_for_closure = libraries.clone();
    
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        match res {
            Ok(event) => {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    for path in event.paths {
                        if let Some((library_id, _)) = libraries_for_closure.iter().find(|(_, root)| {
                            path.starts_with(root)
                        }) {
                            let path_str = path.to_string_lossy().to_string();
                            if parsers::detect_format(&path).is_some() {
                                let _ = tx.try_send((*library_id, path_str));
                            }
                        }
                    }
                }
            }
            Err(err) => error!("[WATCHER] Event error: {}", err),
        }
    })?;

    // Actually watch the library directories
    for (_, root_path) in &libraries {
        info!("[WATCHER] Watching directory: {}", root_path);
        watcher.watch(std::path::Path::new(root_path), RecursiveMode::Recursive)?;
    }

    Ok(watcher)
}
