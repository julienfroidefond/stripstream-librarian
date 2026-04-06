use super::*;
use utoipa::OpenApi;

#[test]
fn test_openapi_generation() {
    let api_doc = ApiDoc::openapi();
    let json = api_doc
        .to_pretty_json()
        .expect("Failed to serialize OpenAPI");

    // Check that all $ref targets exist in components/schemas
    let doc: serde_json::Value =
        serde_json::from_str(&json).expect("OpenAPI JSON should be valid");
    let empty = serde_json::Map::new();
    let schemas = doc["components"]["schemas"]
        .as_object()
        .unwrap_or(&empty);
    let prefix = "#/components/schemas/";
    let mut broken: Vec<String> = Vec::new();
    for part in json.split(prefix).skip(1) {
        if let Some(name) = part.split('"').next() {
            if !schemas.contains_key(name) {
                broken.push(name.to_string());
            }
        }
    }
    broken.dedup();
    assert!(broken.is_empty(), "Unresolved schema refs: {:?}", broken);

    // Save to file for inspection
    std::fs::write("/tmp/openapi.json", &json).expect("Failed to write file");
    println!("OpenAPI JSON saved to /tmp/openapi.json");
}
