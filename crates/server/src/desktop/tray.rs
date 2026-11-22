//! The tray menu's download actions (AC-9).

use krake_core::coordinator::Coordinator;

/// Pause every queued or running download; returns how many were paused.
pub async fn pause_all(coordinator: &Coordinator) -> Result<usize, krake_core::Error> {
    let mut paused = 0;
    for row in coordinator.storage().list_downloads().await? {
        if matches!(row.status.as_str(), "queued" | "downloading") {
            coordinator.pause(&row.id).await?;
            paused += 1;
        }
    }
    Ok(paused)
}

/// Resume every paused download; returns how many were resumed.
pub async fn resume_all(coordinator: &Coordinator) -> Result<usize, krake_core::Error> {
    let mut resumed = 0;
    for row in coordinator.storage().list_downloads().await? {
        if row.status == "paused" {
            coordinator.resume(&row.id).await?;
            resumed += 1;
        }
    }
    Ok(resumed)
}
