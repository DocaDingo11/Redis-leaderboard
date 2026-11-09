use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    db: PgPool,
    redis: redis::Client, // Active Redis client
}

#[derive(Deserialize)]
struct ScoreSubmission {
    player_id: String,
    score: i64,
}

#[derive(Serialize)]
struct LeaderboardEntry {
    player_id: String,
    score: i64,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");

    let db = PgPool::connect(&database_url).await.expect("Failed to connect to Postgres");
    let redis_client = redis::Client::open(redis_url).expect("Failed to create Redis client");

    let state = Arc::new(AppState { db, redis: redis_client });

    let app = Router::new()
        .route("/scores", post(post_score))
        .route("/leaderboard", get(get_leaderboard))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("🚀 Leaderboard engine running on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}

// 1. POST /scores: Save to Postgres audit log + update Redis ZSET
async fn post_score(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScoreSubmission>,
) -> Result<StatusCode, StatusCode> {
    // Save historical record to PostgreSQL
    sqlx::query("INSERT INTO scores (player_id, score) VALUES ($1, $2)")
        .bind(&payload.player_id)
        .bind(payload.score)
        .execute(&state.db)
        .await
        .map_err(|e| {
            eprintln!("Postgres error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Update Redis Sorted Set (ZSET)
// In post_score and get_leaderboard:
let mut con = state.redis.get_multiplexed_async_connection().await.map_err(|e| {
    eprintln!("Redis connection error: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
})?;

    // ZADD leaderboard <score> <player_id>
    let _: usize = con.zadd("leaderboard", &payload.player_id, payload.score).await.map_err(|e| {
        eprintln!("Redis ZADD error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::CREATED)
}

// 2. GET /leaderboard: Fetch Top 10 players instantly from Redis
async fn get_leaderboard(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<LeaderboardEntry>>, StatusCode> {
    let mut con = state.redis.get_multiplexed_async_connection().await.map_err(|e| {
        eprintln!("Redis connection error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // ZREVRANGE leaderboard 0 9 WITHSCORES (gets top 10 descending)
    let results: Vec<(String, i64)> = redis::cmd("ZREVRANGE")
        .arg("leaderboard")
        .arg(0)
        .arg(9)
        .arg("WITHSCORES")
        .query_async(&mut con)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let leaderboard = results
        .into_iter()
        .map(|(player_id, score)| LeaderboardEntry { player_id, score })
        .collect();

    Ok(Json(leaderboard))
}