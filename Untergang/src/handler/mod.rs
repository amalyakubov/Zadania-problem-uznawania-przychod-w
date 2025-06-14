use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use sqlx::{Pool, Postgres};

use crate::client::{Client, ClientId};

pub async fn create_client(
    State(pool): State<Pool<Postgres>>,
    Json(client): Json<Client>,
) -> Result<StatusCode, StatusCode> {
    match client {
        Client::Individual(individual) => {
            let result = sqlx::query!(
                "INSERT INTO personal_clients (first_name, last_name, email, phone_number, pesel) VALUES ($1, $2, $3, $4, $5)",
                individual.first_name,
                individual.last_name,
                individual.email,
                individual.phone_number,
                individual.pesel,
            )
            .execute(&pool)
            .await;
            match result {
                Ok(_) => Ok(StatusCode::CREATED),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Client::Company(company) => {
            let result = sqlx::query!(
                "INSERT INTO company_clients (name, address, email, phone_number, krs) VALUES ($1, $2, $3, $4, $5)",
                company.name,
                company.address,
                company.email,
                company.phone_number,
                company.krs,
            )
            .execute(&pool)
            .await;
            match result {
                Ok(_) => Ok(StatusCode::CREATED),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

// TODO: Prepare migrations for this
pub async fn delete_client(
    State(pool): State<Pool<Postgres>>,
    Json(client_id): Json<ClientId>,
) -> Result<StatusCode, StatusCode> {
    match client_id {
        ClientId::Individual(pesel) => {
            let result = sqlx::query!(
                r#"UPDATE personal_clients
                 SET is_deleted = true, first_name = null, last_name = null, email = null, phone_number = null, pesel = null, created_at = null
                 WHERE pesel = $1"#,
                client_id.0,
            )
            .execute(&pool)
            .await;
            match result {
                Ok(_) => Ok(StatusCode::OK),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        ClientId::Company(krs) => Err(StatusCode::BAD_REQUEST),
    }
}

// TODO: Prepare migrations for this
pub async fn update_client(
    State(pool): State<Pool<Postgres>>,
    Json(client): Json<Client>,
) -> Result<StatusCode, StatusCode> {
    match client {
        Client::Individual(individual) => {
            let result = sqlx::query!(
                "UPDATE personal_clients SET first_name = $1, last_name = $2, email = $3, phone_number = $4 WHERE pesel = $5",
                individual.first_name,
                individual.last_name,
                individual.email,
                individual.phone_number,
                individual.pesel,
            )
            .execute(&pool)
            .await;
            match result {
                Ok(_) => Ok(StatusCode::OK),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Client::Company(company) => {
            let result = sqlx::query!("UPDATE company_clients SET name = $1, address = $2, email = $3, phone_number = $4 WHERE krs = $5",
                company.name,
                company.address,
                company.email,
                company.phone_number,
                company.krs,
            )
            .execute(&pool)
            .await;
            match result {
                Ok(_) => Ok(StatusCode::OK),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}
