use sqlx::PgPool;

#[test]
fn setup() {
    std::env::set_var(
        "DATABASE_URL",
        "postgres://postgres:password@localhost:5432/Untergang", // Use a separate test DB
    );
}

#[sqlx::test]
async fn test_db_connection(pool: PgPool) -> sqlx::Result<()> {
    let _result = sqlx::query("SELECT 1 as test_value")
        .fetch_one(&pool)
        .await?;

    eprintln!("Database connection successful!");
    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_create_contract(pool: PgPool) -> sqlx::Result<()> {
    let table_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_name = 'private_contract'
        )",
    )
    .fetch_one(&pool)
    .await?;

    eprintln!("Table exists: {}", table_exists);

    assert!(table_exists, "Contracts table should exist after migration");
    Ok(())
}

#[cfg(test)]
mod endpoint_tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
        routing::{delete, get, post, put},
        Router,
    };
    use serde_json::json;
    use tower::ServiceExt;

    // Helper function to create test app
    async fn app(pool: PgPool) -> Router {
        Router::new()
            .route("/health", get(|| async { "Status: OK" }))
            .route("/client", post(crate::handler::create_client))
            .route("/client", delete(crate::handler::delete_client))
            .route("/client", put(crate::handler::update_client))
            .route("/contract", post(crate::handler::create_contract))
            .route("/payment", post(crate::handler::create_payment))
            .with_state(pool)
    }

    // Helper function to setup test data
    async fn setup_test_data(pool: &PgPool) -> sqlx::Result<()> {
        // Insert test software product
        sqlx::query(
            "INSERT INTO software (id, name, description, version, category, price) 
             VALUES (1, 'Test Software', 'Test Description', '1.0', 'Test Category', 1000.00)
             ON CONFLICT (id) DO NOTHING",
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_health_endpoint(pool: PgPool) -> sqlx::Result<()> {
        let app = app(pool.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .method(Method::GET)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body_bytes[..], b"Status: OK");

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_create_individual_client(pool: PgPool) -> sqlx::Result<()> {
        let app = app(pool.clone()).await;

        let client = json!({
            "type": "individual",
            "first_name": "John",
            "last_name": "Doe",
            "email": "john.doe@example.com",
            "phone_number": "+48123456789",
            "pesel": "12345678901"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/client")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&client).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        // Verify client was created in database
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM personal_client WHERE pesel = $1)",
        )
        .bind("12345678901")
        .fetch_one(&pool)
        .await?;

        assert!(exists);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_create_company_client(pool: PgPool) -> sqlx::Result<()> {
        let app = app(pool.clone()).await;

        let client = json!({
            "type": "company",
            "name": "Test Company",
            "address": "123 Test Street",
            "email": "company@example.com",
            "phone_number": "+48987654321",
            "krs": "1234567890"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/client")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&client).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        // Verify client was created in database
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM company_client WHERE krs = $1)",
        )
        .bind("1234567890")
        .fetch_one(&pool)
        .await?;

        assert!(exists);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_delete_individual_client(pool: PgPool) -> sqlx::Result<()> {
        // First create a client
        sqlx::query(
            "INSERT INTO personal_client (first_name, last_name, email, phone_number, pesel) 
             VALUES ('Jane', 'Doe', 'jane@example.com', '+48111222333', '98765432109')",
        )
        .execute(&pool)
        .await?;

        let app = app(pool.clone()).await;

        let client_id = json!({
            "type": "individual",
            "value": "98765432109"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/client")
                    .method(Method::DELETE)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&client_id).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Verify client was soft deleted
        let is_deleted = sqlx::query_scalar::<_, bool>(
            "SELECT is_deleted FROM personal_client WHERE pesel = $1",
        )
        .bind("98765432109")
        .fetch_one(&pool)
        .await?;

        assert!(is_deleted);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_delete_company_client_should_fail(pool: PgPool) -> sqlx::Result<()> {
        let app = app(pool.clone()).await;

        let client_id = json!({
            "type": "company",
            "value": "9876543210"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/client")
                    .method(Method::DELETE)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&client_id).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_update_client(pool: PgPool) -> sqlx::Result<()> {
        // First create a client
        sqlx::query(
            "INSERT INTO personal_client (first_name, last_name, email, phone_number, pesel) 
             VALUES ('Old', 'Name', 'old@example.com', '+48000000000', '11111111111')",
        )
        .execute(&pool)
        .await?;

        let app = app(pool.clone()).await;

        let updated_client = json!({
            "type": "individual",
            "first_name": "New",
            "last_name": "Name",
            "email": "new@example.com",
            "phone_number": "+48999999999",
            "pesel": "11111111111"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/client")
                    .method(Method::PUT)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&updated_client).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_create_contract(pool: PgPool) -> sqlx::Result<()> {
        setup_test_data(&pool).await?;

        // Create a client first
        sqlx::query(
            "INSERT INTO personal_client (first_name, last_name, email, phone_number, pesel) 
             VALUES ('Contract', 'Test', 'contract@example.com', '+48555666777', '22222222222')",
        )
        .execute(&pool)
        .await?;

        let app = app(pool.clone()).await;

        let purchase_request = json!({
            "client_id": {
                "type": "individual",
                "value": "22222222222"
            },
            "start_date": "2024-01-01T00:00:00Z",
            "end_date": "2025-01-01T00:00:00Z",
            "product_id": 1,
            "years_supported": 1
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/contract")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&purchase_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        // Verify contract was created
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM private_contract WHERE client_id = $1 AND product_id = $2)"
        )
        .bind("22222222222")
        .bind(1)
        .fetch_one(&pool)
        .await?;

        assert!(exists);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_create_contract_duplicate_should_fail(pool: PgPool) -> sqlx::Result<()> {
        setup_test_data(&pool).await?;

        // Create a client and contract first
        sqlx::query(
            "INSERT INTO personal_client (first_name, last_name, email, phone_number, pesel) 
             VALUES ('Duplicate', 'Test', 'dup@example.com', '+48333444555', '33333333333')",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "INSERT INTO private_contract (client_id, product_id, price, start_date, end_date, years_supported) 
             VALUES ('33333333333', 1, 1000.00, '2024-01-01', '2025-01-01', 1)"
        )
        .execute(&pool)
        .await?;

        let app = app(pool.clone()).await;

        let purchase_request = json!({
            "client_id": {
                "type": "individual",
                "value": "33333333333"
            },
            "start_date": "2024-01-01T00:00:00Z",
            "end_date": "2025-01-01T00:00:00Z",
            "product_id": 1,
            "years_supported": 1
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/contract")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&purchase_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_create_payment_single(pool: PgPool) -> sqlx::Result<()> {
        setup_test_data(&pool).await?;

        // Create a client and contract
        sqlx::query(
            "INSERT INTO personal_client (first_name, last_name, email, phone_number, pesel) 
             VALUES ('Payment', 'Test', 'payment@example.com', '+48777888999', '44444444444')",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "INSERT INTO private_contract (id, client_id, product_id, price, start_date, end_date, years_supported) 
             VALUES (1, '44444444444', 1, 1000.00, '2024-01-01', '2025-01-01', 1)"
        )
        .execute(&pool)
        .await?;

        let app = app(pool.clone()).await;

        let payment_request = json!({
            "SinglePayment": {
                "contract_id": 1,
                "amount": 1000.0,
                "client_id": {
                    "type": "individual",
                    "value": "44444444444"
                }
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/payment")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&payment_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Verify payment was recorded
        let is_paid =
            sqlx::query_scalar::<_, bool>("SELECT is_paid FROM private_contract WHERE id = $1")
                .bind(1)
                .fetch_one(&pool)
                .await?;

        assert!(is_paid);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_create_payment_installments(pool: PgPool) -> sqlx::Result<()> {
        setup_test_data(&pool).await?;

        // Create a client and contract
        sqlx::query(
            "INSERT INTO personal_client (first_name, last_name, email, phone_number, pesel) 
             VALUES ('Installment', 'Test', 'installment@example.com', '+48123123123', '55555555555')"
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "INSERT INTO private_contract (id, client_id, product_id, price, start_date, end_date, years_supported) 
             VALUES (2, '55555555555', 1, 1200.00, '2024-01-01', '2025-01-01', 1)"
        )
        .execute(&pool)
        .await?;

        let app = app(pool.clone()).await;

        let payment_request = json!({
            "Installments": {
                "contract_id": 2,
                "client_id": {
                    "type": "individual",
                    "value": "55555555555"
                },
                "amount_per_installment": 100.0,
                "amount_of_installments": 12
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/payment")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&payment_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Verify payment was recorded
        let is_paid =
            sqlx::query_scalar::<_, bool>("SELECT is_paid FROM private_contract WHERE id = $1")
                .bind(2)
                .fetch_one(&pool)
                .await?;

        assert!(is_paid);

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_payment_wrong_amount_should_fail(pool: PgPool) -> sqlx::Result<()> {
        setup_test_data(&pool).await?;

        // Create a client and contract
        sqlx::query(
            "INSERT INTO personal_client (first_name, last_name, email, phone_number, pesel) 
             VALUES ('Wrong', 'Amount', 'wrong@example.com', '+48999000111', '66666666666')",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "INSERT INTO private_contract (id, client_id, product_id, price, start_date, end_date, years_supported) 
             VALUES (3, '66666666666', 1, 1000.00, '2024-01-01', '2025-01-01', 1)"
        )
        .execute(&pool)
        .await?;

        let app = app(pool.clone()).await;

        let payment_request = json!({
            "SinglePayment": {
                "contract_id": 3,
                "amount": 500.0,  // Wrong amount
                "client_id": {
                    "type": "individual",
                    "value": "66666666666"
                }
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/payment")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&payment_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        Ok(())
    }
}
