use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// Simple acknowledgment response.
#[derive(Debug, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
}

impl OkResponse {
    pub fn new() -> Self {
        Self { ok: true }
    }
}

/// Response for resource deletion operations.
#[derive(Debug, Serialize, ToSchema)]
pub struct DeletedResponse {
    pub deleted: bool,
    pub id: Uuid,
}

impl DeletedResponse {
    pub fn new(id: Uuid) -> Self {
        Self { deleted: true, id }
    }
}

/// Response for resource update operations.
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdatedResponse {
    pub updated: bool,
    pub id: Uuid,
}

impl UpdatedResponse {
    pub fn new(id: Uuid) -> Self {
        Self { updated: true, id }
    }
}

/// Response for token revocation.
#[derive(Debug, Serialize, ToSchema)]
pub struct RevokedResponse {
    pub revoked: bool,
    pub id: Uuid,
}

impl RevokedResponse {
    pub fn new(id: Uuid) -> Self {
        Self { revoked: true, id }
    }
}

/// Response for unlinking operations (e.g., AniList).
#[derive(Debug, Serialize, ToSchema)]
pub struct UnlinkedResponse {
    pub unlinked: bool,
}

impl UnlinkedResponse {
    pub fn new() -> Self {
        Self { unlinked: true }
    }
}

/// Simple status response.
#[derive(Debug, Serialize, ToSchema)]
pub struct StatusResponse {
    pub status: String,
}

impl StatusResponse {
    pub fn new(status: &str) -> Self {
        Self {
            status: status.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "tests/responses.rs"]
mod tests;
