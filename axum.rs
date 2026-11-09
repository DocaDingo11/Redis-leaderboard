async fn ingest_event(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SecurityEvent>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query(
        "INSERT INTO audit_logs (event_type, severity, source_ip) VALUES ($1, $2, $3)"
    )
    .bind(&payload.event_type)
    .bind(payload.severity as i32)
    .bind(&payload.source_ip)
    .execute(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Database insert failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::CREATED)
}