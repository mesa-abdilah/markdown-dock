use tauri::State;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::indexer::search::{self, DocSnippet, SearchHit};

#[tauri::command]
#[specta::specta]
pub async fn search_docs(
    state: State<'_, AppState>,
    query: String,
    root_id: Option<i32>,
    limit: Option<u32>,
) -> Result<Vec<SearchHit>, AppError> {
    let db = state.db.clone();
    let root_filter = root_id.map(i64::from);
    let max_hits = limit.unwrap_or(100) as usize;

    tokio::task::spawn_blocking(move || {
        search::search_documents(&db, &query, root_filter, max_hits)
    })
    .await
    .map_err(|e| AppError::Db(format!("Search task failed: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn get_snippets(
    state: State<'_, AppState>,
    doc_ids: Vec<i32>,
    query: String,
) -> Result<Vec<DocSnippet>, AppError> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || search::get_snippets(&db, &doc_ids, &query))
        .await
        .map_err(|e| AppError::Db(format!("Snippet task failed: {e}")))?
}
