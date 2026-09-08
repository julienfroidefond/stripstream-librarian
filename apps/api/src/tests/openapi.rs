use super::*;
use utoipa::OpenApi;

fn check_openapi_spec(doc: utoipa::openapi::OpenApi, name: &str) {
    let json = doc
        .to_pretty_json()
        .unwrap_or_else(|_| panic!("Failed to serialize {} OpenAPI", name));

    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("OpenAPI JSON should be valid");
    let empty = serde_json::Map::new();
    let schemas = parsed["components"]["schemas"]
        .as_object()
        .unwrap_or(&empty);
    let prefix = "#/components/schemas/";
    let mut broken: Vec<String> = Vec::new();
    for part in json.split(prefix).skip(1) {
        if let Some(ref_name) = part.split('"').next() {
            if !schemas.contains_key(ref_name) {
                broken.push(ref_name.to_string());
            }
        }
    }
    broken.dedup();
    assert!(
        broken.is_empty(),
        "{} — Unresolved schema refs: {:?}",
        name,
        broken
    );

    let path = format!("/tmp/openapi_{}.json", name);
    std::fs::write(&path, &json).expect("Failed to write file");
    println!("{} OpenAPI saved to {}", name, path);
}

#[test]
fn test_client_openapi_generation() {
    check_openapi_spec(ClientApiDoc::openapi(), "client");
}

#[test]
fn test_admin_openapi_generation() {
    check_openapi_spec(AdminApiDoc::openapi(), "admin");
}

#[test]
fn client_openapi_exposes_favorite_and_rating_mutations() {
    let spec = serde_json::to_value(ClientApiDoc::openapi()).unwrap();

    for (path, methods) in [
        (
            "/series/{series_id}/favorite",
            &["get", "put", "delete"][..],
        ),
        ("/series/{series_id}/rating", &["put", "delete"][..]),
    ] {
        for &method in methods {
            assert!(
                spec["paths"][path][method].is_object(),
                "client OpenAPI must expose {method} {path}"
            );
        }
    }
}
