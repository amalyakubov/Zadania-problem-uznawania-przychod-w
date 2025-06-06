use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use sqlx::{Pool, Postgres};

use crate::client::Client;

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
