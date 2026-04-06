use super::*;

#[test]
fn api_job_types_contains_metadata_refresh_all() {
    assert!(
        API_JOB_TYPES.contains(&"metadata_refresh_all"),
        "API_JOB_TYPES must include metadata_refresh_all"
    );
}

#[test]
fn api_job_types_contains_all_expected_types() {
    let expected = &[
        "metadata_batch",
        "metadata_refresh",
        "metadata_refresh_all",
        "reading_status_push",
        "download_detection",
    ];
    for t in expected {
        assert!(
            API_JOB_TYPES.contains(t),
            "API_JOB_TYPES is missing: {t}"
        );
    }
}
