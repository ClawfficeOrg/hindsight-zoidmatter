use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

use hindsight_core::MemoryType;
use hindsight_missions::{FactStore, InMemoryFactStore, MemoryItem};
use hindsight_recall::{detect_conflicts, RecallPipeline};

// ── Shared app state ────────────────────────────────────────────────────────

type Db = Arc<Mutex<InMemoryFactStore>>;

struct AppState {
    store: Db,
    pipeline: RecallPipeline,
}

// ── Ingest models ──────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct RetainRequest {
    pub documents: Vec<RetainDocument>,
}

#[derive(serde::Deserialize)]
struct RetainDocument {
    pub id: Option<String>,
    pub content: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
struct RetainResponse {
    pub status: String,
    pub document_ids: Vec<String>,
}

// ── Recall models ──────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct RecallRequest {
    pub query: String,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
    pub project: Option<String>,
}

#[derive(serde::Serialize)]
struct RecallItem {
    pub id: String,
    pub name: String,
    pub content: String,
    pub memory_type: String,
    pub score: Option<f64>,
}

#[derive(serde::Serialize)]
struct RecallResponse {
    pub results: Vec<RecallItem>,
}

// ── Conflict detection models ──────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct ConflictRequest {
    pub items: Vec<String>,
}

#[derive(serde::Serialize)]
struct ConflictInfo {
    pub item_a_id: String,
    pub item_b_id: String,
    pub subject: String,
}

#[derive(serde::Serialize)]
struct ConflictResponse {
    pub conflicts: Vec<ConflictInfo>,
    pub resolved_items: Vec<RecallItem>,
}

// ── Handlers ────────────────────────────────────────────────────────────────

async fn health_check() -> &'static str {
    "ok"
}

async fn retain(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RetainRequest>,
) -> Json<RetainResponse> {
    let mut store = state.store.lock().await;
    let mut ids = Vec::new();

    for doc in &req.documents {
        let id = doc
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let name = doc
            .metadata
            .as_ref()
            .and_then(|m| m.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();

        let memory_type = doc
            .metadata
            .as_ref()
            .and_then(|m| m.get("memory_type"))
            .and_then(|v| v.as_str())
            .map(|s| match s.to_lowercase().as_str() {
                "architectural_invariant" | "architecturalinvariant" => {
                    MemoryType::ArchitecturalInvariant
                }
                "hard_fact" | "hardfact" => MemoryType::HardFact,
                "explicit_mental_model" | "explicitmentalmodel" => MemoryType::ExplicitMentalModel,
                "empirical_observation" | "empiricalobservation" => {
                    MemoryType::EmpiricalObservation
                }
                "conversational_context" | "conversationalcontext" => {
                    MemoryType::ConversationalContext
                }
                _ => MemoryType::EmpiricalObservation,
            })
            .unwrap_or(MemoryType::EmpiricalObservation);

        let item = MemoryItem::new(&id, &name, &doc.content, memory_type, "api");
        store.store(item).ok();
        ids.push(id);
    }

    Json(RetainResponse {
        status: "stored".to_string(),
        document_ids: ids,
    })
}

async fn recall(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RecallRequest>,
) -> Json<RecallResponse> {
    let store = state.store.lock().await;
    let all_items = store.all_items().unwrap_or_default();

    // Run the recall pipeline (invariant pre-check gate + dedup)
    let from_pipeline = state
        .pipeline
        .invariant_pre_check_gate(all_items, &*store, req.project.as_deref())
        .unwrap_or_default();

    let top_k = req.top_k.unwrap_or(10);
    let items: Vec<RecallItem> = from_pipeline
        .into_iter()
        .take(top_k)
        .map(|i| RecallItem {
            id: i.id,
            name: i.name,
            content: i.content,
            memory_type: format!("{:?}", i.memory_type),
            score: None,
        })
        .collect();

    Json(RecallResponse { results: items })
}

async fn conflict_detect(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConflictRequest>,
) -> Json<ConflictResponse> {
    let store = state.store.lock().await;
    let all = store.all_items().unwrap_or_default();

    let target_ids: HashSet<&str> = req.items.iter().map(|s| s.as_str()).collect();
    let relevant: Vec<MemoryItem> = all
        .into_iter()
        .filter(|i| target_ids.contains(i.id.as_str()))
        .collect();

    let resolution = detect_conflicts(relevant);

    let conflicts: Vec<ConflictInfo> = resolution
        .conflicts
        .iter()
        .map(|c| ConflictInfo {
            item_a_id: c.item_a_id.clone(),
            item_b_id: c.item_b_id.clone(),
            subject: c.subject.clone(),
        })
        .collect();

    let resolved_items: Vec<RecallItem> = resolution
        .resolved
        .into_iter()
        .map(|i| RecallItem {
            id: i.id,
            name: i.name,
            content: i.content,
            memory_type: format!("{:?}", i.memory_type),
            score: None,
        })
        .collect();

    Json(ConflictResponse {
        conflicts,
        resolved_items,
    })
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let store = Arc::new(Mutex::new(InMemoryFactStore::default()));
    let pipeline = RecallPipeline::new();
    let state = Arc::new(AppState { store, pipeline });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/v1/retain", post(retain))
        .route("/v1/recall", post(recall))
        .route("/v1/conflicts", post(conflict_detect))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8888".to_string())
        .parse()
        .unwrap_or(8888);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("ZoidMatter API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
