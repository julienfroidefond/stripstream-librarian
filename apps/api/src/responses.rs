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
mod tests {
    use super::*;

    #[test]
    fn ok_response_serializes() {
        let r = OkResponse::new();
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json, serde_json::json!({"ok": true}));
    }

    #[test]
    fn deleted_response_serializes() {
        let id = Uuid::nil();
        let r = DeletedResponse::new(id);
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["deleted"], true);
        assert_eq!(json["id"], id.to_string());
    }

    #[test]
    fn updated_response_serializes() {
        let id = Uuid::nil();
        let r = UpdatedResponse::new(id);
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["updated"], true);
        assert_eq!(json["id"], id.to_string());
    }

    #[test]
    fn revoked_response_serializes() {
        let id = Uuid::nil();
        let r = RevokedResponse::new(id);
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["revoked"], true);
        assert_eq!(json["id"], id.to_string());
    }

    #[test]
    fn unlinked_response_serializes() {
        let r = UnlinkedResponse::new();
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json, serde_json::json!({"unlinked": true}));
    }

    #[test]
    fn status_response_serializes() {
        let r = StatusResponse::new("ready");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json, serde_json::json!({"status": "ready"}));
    }
}
