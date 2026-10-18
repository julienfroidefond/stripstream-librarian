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
fn client_openapi_exposes_book_file_download() {
    let spec = serde_json::to_value(ClientApiDoc::openapi()).unwrap();

    assert!(
        spec["paths"]["/books/{book_id}/file"]["get"].is_object(),
        "client OpenAPI must expose GET /books/{{book_id}}/file"
    );
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

/// AllDebrid and qBittorrent are admin-only integrations: their endpoints must
/// never leak into the read-scoped specification the reader client consumes.
#[test]
fn client_openapi_exposes_no_download_client_routes() {
    let spec = serde_json::to_value(ClientApiDoc::openapi()).unwrap();
    let paths = spec["paths"].as_object().expect("paths must be an object");

    let leaked: Vec<&String> = paths
        .keys()
        .filter(|path| path.contains("alldebrid") || path.contains("qbittorrent"))
        .collect();
    assert!(
        leaked.is_empty(),
        "download clients are admin-only, but the client spec exposes: {leaked:?}"
    );
}

#[test]
fn admin_openapi_exposes_the_alldebrid_routes() {
    let spec = serde_json::to_value(AdminApiDoc::openapi()).unwrap();
    let paths = spec["paths"].as_object().expect("paths must be an object");

    for path in ["/alldebrid/test", "/alldebrid/add"] {
        assert!(paths.contains_key(path), "admin OpenAPI must expose {path}");
    }
}
