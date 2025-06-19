use axum::{
    extract::Json,
    http::StatusCode,
    routing::{delete, get, post, put},
    Router,
};

mod client;
use client::{Client, ClientId};

mod db;
use db::{connect_db, initialize_db};

mod handler;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let pool = connect_db().await.unwrap();
    match initialize_db(&pool).await {
        Ok(_) => println!("Database initialized"),
        Err(e) => println!("Error initializing database: {}", e),
    }

    // build our application with a route
    let app = Router::new()
        .route("/health", get(|| async { "Status: OK" }))
        // POST /client
        .route("/client", post(handler::create_client))
        // DELETE /client
        .route("/client", delete(handler::delete_client))
        // PUT /client TODO: add update functionality
        .route(
            "/client",
            put(move |Json(client): Json<Client>| async move { (StatusCode::OK, Json(client)) }),
        )
        .route("/contract", post(handler::create_contract))
        .with_state(pool);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    println!("Server is running on port 3000");
    axum::serve(listener, app).await.unwrap();
}
