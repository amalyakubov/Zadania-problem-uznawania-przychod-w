use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};

use crate::{
    client::{Client, ClientId},
    db::{
        check_if_client_has_contract_for_product, check_product_and_client_exist,
        create_contract_in_db, find_discounts_for_client, get_price_for_product,
    },
};

pub enum AppError {
    BadRequest(String),
    InternalServerError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::InternalServerError(msg) => {
                eprintln!("Internal Server Error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal server error occurred".to_string(),
                )
            }
        };

        (status, error_message).into_response()
    }
}

pub async fn create_client(
    State(pool): State<Pool<Postgres>>,
    Json(client): Json<Client>,
) -> Result<(StatusCode, String), AppError> {
    let result = match client {
        Client::Individual(individual) => {
            sqlx::query!(
                "INSERT INTO personal_client (first_name, last_name, email, phone_number, pesel) VALUES ($1, $2, $3, $4, $5)",
                individual.first_name,
                individual.last_name,
                individual.email,
                individual.phone_number,
                individual.pesel,
            )
            .execute(&pool)
            .await
        }
        Client::Company(company) => {
            sqlx::query!(
                "INSERT INTO company_client (name, address, email, phone_number, krs) VALUES ($1, $2, $3, $4, $5)",
                company.name,
                company.address,
                company.email,
                company.phone_number,
                company.krs,
            )
            .execute(&pool)
            .await
        }
    };

    match result {
        Ok(_) => Ok((StatusCode::CREATED, "Client created".to_string())),
        Err(e) => Err(AppError::InternalServerError(format!(
            "Failed to create client: {}",
            e
        ))),
    }
}

// TODO: Prepare migrations for this
pub async fn delete_client(
    State(pool): State<Pool<Postgres>>,
    Json(client_id): Json<ClientId>,
) -> Result<(StatusCode, String), AppError> {
    match client_id {
        ClientId::Individual(pesel) => {
            sqlx::query!(
                r#"UPDATE personal_client
                 SET is_deleted = true, first_name = null, last_name = null, email = null, phone_number = null, pesel = null, created_at = null
                 WHERE pesel = $1"#,
                pesel,
            )
            .execute(&pool)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to delete client: {}", e)))?;
            Ok((StatusCode::OK, "Client deleted".to_string()))
        }
        ClientId::Company(_krs) => Err(AppError::BadRequest(
            "Failed to delete client: coroprate clients are unable to be deleted".to_string(),
        )),
    }
}

// TODO: Prepare migrations for this
pub async fn update_client(
    State(pool): State<Pool<Postgres>>,
    Json(client): Json<Client>,
) -> Result<(StatusCode, String), AppError> {
    let result = match client {
        Client::Individual(individual) => {
            sqlx::query!(
                "UPDATE personal_client SET first_name = $1, last_name = $2, email = $3, phone_number = $4 WHERE pesel = $5",
                individual.first_name,
                individual.last_name,
                individual.email,
                individual.phone_number,
                individual.pesel,
            )
            .execute(&pool)
            .await
        }
        Client::Company(company) => {
            sqlx::query!("UPDATE company_client SET name = $1, address = $2, email = $3, phone_number = $4 WHERE krs = $5",
                company.name,
                company.address,
                company.email,
                company.phone_number,
                company.krs,
            )
            .execute(&pool)
            .await
        }
    };

    match result {
        Ok(_) => Ok((StatusCode::OK, "Client updated".to_string())),
        Err(e) => Err(AppError::InternalServerError(format!(
            "Failed to update client: {}",
            e
        ))),
    }
}

#[derive(serde::Deserialize)]
pub struct PurchaseRequest {
    client_id: ClientId,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    product_id: i32,
    // price is calculated on the backen
    // update information is availbable in the database
    years_supported: i32, // every year costs 1 000 additional zł, can be extended by 1, 2, 3 years
}

pub async fn create_contract(
    State(pool): State<Pool<Postgres>>,
    Json(purchase_request): Json<PurchaseRequest>,
) -> Result<(StatusCode, String), AppError> {
    // check if the client hasn't already ordered the product
    let client_has_contract = check_if_client_has_contract_for_product(
        &pool,
        purchase_request.client_id.clone(),
        purchase_request.product_id,
    )
    .await
    .map_err(|e| {
        AppError::InternalServerError(format!("Failed to check if client has contract: {}", e))
    })?;

    if client_has_contract {
        return Err(AppError::BadRequest(
            "Client already has contract for this product".to_string(),
        ));
    }

    // check if product and client exist
    let (product_exists, client_exists) = check_product_and_client_exist(
        &pool,
        purchase_request.product_id,
        purchase_request.client_id.clone(),
    )
    .await
    .map_err(|e| {
        AppError::InternalServerError(format!(
            "Failed to check if product and client exist: {}",
            e
        ))
    })?;

    if !product_exists {
        return Err(AppError::BadRequest("Product does not exist".to_string()));
    }
    if !client_exists {
        return Err(AppError::BadRequest("Client does not exist".to_string()));
    }

    // get discount for client
    let discount = find_discounts_for_client(
        &pool,
        purchase_request.product_id,
        purchase_request.client_id.clone(),
    )
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to get discount: {}", e)))?
    .unwrap_or(0.0);

    let price = get_price_for_product(&pool, purchase_request.product_id)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to get price: {:?}", e)))?;

    let final_price = price * (1.0 - discount);

    create_contract_in_db(
        &pool,
        final_price,
        purchase_request.product_id,
        purchase_request.client_id.clone(),
        purchase_request.start_date,
        purchase_request.end_date,
        purchase_request.years_supported,
    )
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to create contract: {}", e)))?;

    Ok((StatusCode::CREATED, "Contract created".to_string()))
}
