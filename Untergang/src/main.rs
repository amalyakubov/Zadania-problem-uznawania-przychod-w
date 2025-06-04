use axum::{
    extract::{Path, Query},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};

mod client;
use client::{Client, ClientId};

mod db;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app =
        Router::new()
            .route("/health", get(|| async { "Status: OKAY" }))
            .route(
                "/client",
                post(move |Json(client): Json<Client>| async move {
                    (StatusCode::CREATED, Json(client))
                }),
            )
            .route(
                "/client",
                delete(move |Json(client): Json<ClientId>| async move {
                    (StatusCode::OK, Json(client))
                }),
            )
            .route(
                "/client",
                put(
                    move |Json(client): Json<Client>| async move { (StatusCode::OK, Json(client)) },
                ),
            );

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    println!("Server is running on port 3000");
    axum::serve(listener, app).await.unwrap();
}
