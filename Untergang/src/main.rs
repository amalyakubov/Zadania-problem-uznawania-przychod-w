use axum::{
    routing::{delete, get, post},
    Router,
};

mod client;

mod db;
use db::connect_db;

#[cfg(test)]
mod tests;

mod handler;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let pool = connect_db().await.unwrap();
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // build our application with a route
    let app = Router::new()
        .route("/health", get(|| async { "Status: OK" }))
        // POST /client
        .route("/client", post(handler::create_client))
        // DELETE /client
        .route("/client", delete(handler::delete_client))
        // PUT /client
        .route("/client", put(handler::update_client))
        // POST /contract
        .route("/contract", post(handler::create_contract))
        .route("/payment", post(handler::create_payment))
        .with_state(pool);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    println!("Server is running on port 3000");
    axum::serve(listener, app).await.unwrap();
}
