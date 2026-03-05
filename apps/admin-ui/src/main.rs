use axum::{
    extract::{Form, State},
    response::{Html, Redirect},
    routing::{get, post},
    Router,
};
use reqwest::Client;
use serde::Deserialize;
use stripstream_core::config::AdminUiConfig;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    api_base_url: String,
    api_token: String,
    client: Client,
}

#[derive(Deserialize)]
struct LibraryDto {
    id: Uuid,
    name: String,
    root_path: String,
    enabled: bool,
}

#[derive(Deserialize)]
struct IndexJobDto {
    id: Uuid,
    #[serde(rename = "type")]
    kind: String,
    status: String,
    created_at: String,
}

#[derive(Deserialize)]
struct TokenDto {
    id: Uuid,
    name: String,
    scope: String,
    prefix: String,
    revoked_at: Option<String>,
}

#[derive(Deserialize)]
struct CreatedToken {
    token: String,
}

#[derive(Deserialize)]
struct AddLibraryForm {
    name: String,
    root_path: String,
}

#[derive(Deserialize)]
struct DeleteLibraryForm {
    id: Uuid,
}

#[derive(Deserialize)]
struct RebuildForm {
    library_id: String,
}

#[derive(Deserialize)]
struct CreateTokenForm {
    name: String,
    scope: String,
}

#[derive(Deserialize)]
struct RevokeTokenForm {
    id: Uuid,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "admin_ui=info,axum=info".to_string()),
        )
        .init();

    let config = AdminUiConfig::from_env()?;
    let state = AppState {
        api_base_url: config.api_base_url,
        api_token: config.api_token,
        client: Client::new(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(index))
        .route("/libraries", get(libraries_page))
        .route("/libraries/add", post(add_library))
        .route("/libraries/delete", post(delete_library))
        .route("/jobs", get(jobs_page))
        .route("/jobs/rebuild", post(rebuild_jobs))
        .route("/tokens", get(tokens_page))
        .route("/tokens/create", post(create_token))
        .route("/tokens/revoke", post(revoke_token))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "admin ui listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn index() -> Html<String> {
    Html(layout(
        "Dashboard",
        "<h1>Stripstream Admin</h1><p>Gestion des librairies, jobs d'indexation et tokens API.</p><p><a href='/libraries'>Libraries</a> | <a href='/jobs'>Jobs</a> | <a href='/tokens'>Tokens</a></p>",
    ))
}

async fn libraries_page(State(state): State<AppState>) -> Html<String> {
    let libraries = fetch_libraries(&state).await.unwrap_or_default();
    let mut rows = String::new();
    for lib in libraries {
        rows.push_str(&format!(
            "<tr><td>{}</td><td><code>{}</code></td><td>{}</td><td><form method='post' action='/libraries/delete'><input type='hidden' name='id' value='{}'/><button type='submit'>Delete</button></form></td></tr>",
            html_escape(&lib.name),
            html_escape(&lib.root_path),
            if lib.enabled { "yes" } else { "no" },
            lib.id
        ));
    }

    let content = format!(
        "<h1>Libraries</h1>
<form method='post' action='/libraries/add'>
  <input name='name' placeholder='Name' required />
  <input name='root_path' placeholder='/libraries/demo' required />
  <button type='submit'>Add</button>
</form>
<table border='1' cellpadding='6'>
  <tr><th>Name</th><th>Root Path</th><th>Enabled</th><th>Actions</th></tr>
  {}
</table>",
        rows
    );

    Html(layout("Libraries", &content))
}

async fn add_library(State(state): State<AppState>, Form(form): Form<AddLibraryForm>) -> Redirect {
    let _ = state
        .client
        .post(format!("{}/libraries", state.api_base_url))
        .bearer_auth(&state.api_token)
        .json(&serde_json::json!({"name": form.name, "root_path": form.root_path}))
        .send()
        .await;
    Redirect::to("/libraries")
}

async fn delete_library(State(state): State<AppState>, Form(form): Form<DeleteLibraryForm>) -> Redirect {
    let _ = state
        .client
        .delete(format!("{}/libraries/{}", state.api_base_url, form.id))
        .bearer_auth(&state.api_token)
        .send()
        .await;
    Redirect::to("/libraries")
}

async fn jobs_page(State(state): State<AppState>) -> Html<String> {
    let jobs = fetch_jobs(&state).await.unwrap_or_default();
    let mut rows = String::new();
    for job in jobs {
        rows.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            job.id,
            html_escape(&job.kind),
            html_escape(&job.status),
            html_escape(&job.created_at)
        ));
    }

    let content = format!(
        "<h1>Index Jobs</h1>
<form method='post' action='/jobs/rebuild'>
  <input name='library_id' placeholder='optional library UUID' />
  <button type='submit'>Queue Rebuild</button>
</form>
<table border='1' cellpadding='6'>
  <tr><th>ID</th><th>Type</th><th>Status</th><th>Created</th></tr>
  {}
</table>",
        rows
    );

    Html(layout("Jobs", &content))
}

async fn rebuild_jobs(State(state): State<AppState>, Form(form): Form<RebuildForm>) -> Redirect {
    let body = if form.library_id.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({"library_id": form.library_id.trim()})
    };
    let _ = state
        .client
        .post(format!("{}/index/rebuild", state.api_base_url))
        .bearer_auth(&state.api_token)
        .json(&body)
        .send()
        .await;
    Redirect::to("/jobs")
}

async fn tokens_page(State(state): State<AppState>) -> Html<String> {
    let tokens = fetch_tokens(&state).await.unwrap_or_default();
    let mut rows = String::new();
    for token in tokens {
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td><td><form method='post' action='/tokens/revoke'><input type='hidden' name='id' value='{}'/><button type='submit'>Revoke</button></form></td></tr>",
            html_escape(&token.name),
            html_escape(&token.scope),
            html_escape(&token.prefix),
            if token.revoked_at.is_some() { "yes" } else { "no" },
            token.id
        ));
    }

    let content = format!(
        "<h1>API Tokens</h1>
<form method='post' action='/tokens/create'>
  <input name='name' placeholder='token name' required />
  <select name='scope'>
    <option value='read'>read</option>
    <option value='admin'>admin</option>
  </select>
  <button type='submit'>Create Token</button>
</form>
<table border='1' cellpadding='6'>
  <tr><th>Name</th><th>Scope</th><th>Prefix</th><th>Revoked</th><th>Actions</th></tr>
  {}
</table>",
        rows
    );

    Html(layout("Tokens", &content))
}

async fn create_token(State(state): State<AppState>, Form(form): Form<CreateTokenForm>) -> Html<String> {
    let response = state
        .client
        .post(format!("{}/admin/tokens", state.api_base_url))
        .bearer_auth(&state.api_token)
        .json(&serde_json::json!({"name": form.name, "scope": form.scope}))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let created: Result<CreatedToken, _> = resp.json().await;
            match created {
                Ok(token) => Html(layout(
                    "Token Created",
                    &format!(
                        "<h1>Token Created</h1><p>Copie ce token maintenant (il ne sera plus affiche):</p><pre>{}</pre><p><a href='/tokens'>Back to tokens</a></p>",
                        html_escape(&token.token)
                    ),
                )),
                Err(_) => Html(layout("Error", "<p>Token created but response parse failed.</p><p><a href='/tokens'>Back</a></p>")),
            }
        }
        _ => Html(layout("Error", "<p>Token creation failed.</p><p><a href='/tokens'>Back</a></p>")),
    }
}

async fn revoke_token(State(state): State<AppState>, Form(form): Form<RevokeTokenForm>) -> Redirect {
    let _ = state
        .client
        .delete(format!("{}/admin/tokens/{}", state.api_base_url, form.id))
        .bearer_auth(&state.api_token)
        .send()
        .await;
    Redirect::to("/tokens")
}

async fn fetch_libraries(state: &AppState) -> Result<Vec<LibraryDto>, reqwest::Error> {
    state
        .client
        .get(format!("{}/libraries", state.api_base_url))
        .bearer_auth(&state.api_token)
        .send()
        .await?
        .json::<Vec<LibraryDto>>()
        .await
}

async fn fetch_jobs(state: &AppState) -> Result<Vec<IndexJobDto>, reqwest::Error> {
    state
        .client
        .get(format!("{}/index/status", state.api_base_url))
        .bearer_auth(&state.api_token)
        .send()
        .await?
        .json::<Vec<IndexJobDto>>()
        .await
}

async fn fetch_tokens(state: &AppState) -> Result<Vec<TokenDto>, reqwest::Error> {
    state
        .client
        .get(format!("{}/admin/tokens", state.api_base_url))
        .bearer_auth(&state.api_token)
        .send()
        .await?
        .json::<Vec<TokenDto>>()
        .await
}

fn layout(title: &str, content: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>{}</title><style>body{{font-family:ui-sans-serif,system-ui;margin:2rem;line-height:1.4}}table{{width:100%;border-collapse:collapse;margin-top:1rem}}th,td{{text-align:left}}input,select,button{{padding:.45rem;margin:.25rem}}</style></head><body><nav><a href='/'>Home</a> | <a href='/libraries'>Libraries</a> | <a href='/jobs'>Jobs</a> | <a href='/tokens'>Tokens</a></nav><hr />{}</body></html>",
        html_escape(title),
        content
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
