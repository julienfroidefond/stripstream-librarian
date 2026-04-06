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
