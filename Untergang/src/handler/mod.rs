use crate::db::{payments, payments::handle_full_payment};
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};

use crate::{
    client::{Client, ClientId},
    db::{
        check_if_client_exists, check_if_client_has_contract_for_product,
        check_product_and_client_exist, create_contract_in_db, find_discounts_for_client,
        get_contract_by_id, get_price_for_product, pay_for_contract,
    },
};

#[derive(Debug)]
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
            sqlx::query(
                r#"UPDATE personal_client
                 SET is_deleted = true
                 WHERE pesel = $1"#,
            )
            .bind(pesel)
            .execute(&pool)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to delete client: {}", e))
            })?;
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

#[derive(Clone, serde::Deserialize)]
pub struct InstallmentsPayment {
    contract_id: i32,
    client_id: ClientId,
    amount: f64,
}

#[derive(Clone, serde::Deserialize)]
pub struct SinglePayment {
    contract_id: i32,
    amount: f64,
    client_id: ClientId,
}

#[derive(Clone, serde::Deserialize)]
pub enum PaymentRequest {
    Installments(InstallmentsPayment),
    SinglePayment(SinglePayment),
}

pub async fn create_payment(
    State(pool): State<Pool<Postgres>>,
    Json(payment_request): Json<PaymentRequest>,
) -> Result<(StatusCode, String), AppError> {
    let (client_id, contract_id) = match payment_request.clone() {
        PaymentRequest::Installments(installments_payment) => (
            installments_payment.client_id.clone(),
            installments_payment.contract_id,
        ),
        PaymentRequest::SinglePayment(single_payment) => {
            (single_payment.client_id, single_payment.contract_id)
        }
    };

    let client_exists = check_if_client_exists(&pool, &client_id)
        .await
        .map_err(|e| {
            AppError::InternalServerError(format!("Failed to check if client exists: {}", e))
        })?;
    if !client_exists {
        return Err(AppError::BadRequest("Client does not exist".to_string()));
    }

    // Try to get the contract - if it doesn't exist or doesn't belong to the client, this will fail
    let contract = get_contract_by_id(&pool, client_id.clone(), contract_id)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::BadRequest(
                "Contract does not exist or does not belong to this client".to_string(),
            ),
            _ => AppError::InternalServerError(format!("Failed to get contract: {}", e)),
        })?;

    if contract.is_paid {
        return Err(AppError::BadRequest("Contract is already paid".to_string()));
    }

    let current_date = Utc::now();
    // if the contract is expired, create a new contract
    if contract.end_date <= current_date {
        // get the outstanding payments
        let outstanding_payments = payments::check_outstanding_payments(&pool, contract_id)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!(
                    "Failed to check outstanding payments: {:?}",
                    e
                ))
            })?;

        payments::create_payment_record_in_db(&pool, contract_id, outstanding_payments * -1.0)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to create payment: {:?}", e))
            })?;

        let new_contract = create_contract_in_db(
            &pool,
            contract
                .price
                .to_f64()
                .expect("Failed to convert price to f64"),
            contract.product_id,
            client_id.clone(),
            contract.start_date,
            contract.end_date,
            contract.years_supported,
        )
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to create contract: {}", e)))?;

        return Ok((
            StatusCode::CREATED,
            "Missed payment, a new contract has been created. Any outstanding payments have been returned to you.".to_string(),
        ));
    }

    match payment_request {
        PaymentRequest::Installments(installments_payment) => {
            let outstanding_payments = payments::check_outstanding_payments(&pool, contract_id)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!(
                        "Failed to check outstanding payments: {:?}",
                        e
                    ))
                })?;

            if installments_payment.amount > outstanding_payments {
                return Err(AppError::BadRequest(
                    "Amount is greater than outstanding payments".to_string(),
                ));
            }

            // Create a payment entry in the database
            pay_for_contract(&pool, contract_id, &client_id, installments_payment.amount)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to pay for contract: {:?}", e))
                })?;

            // If the payment is the full amount, handle the full payment and set the contract to paid =>'signed'
            if installments_payment.amount == outstanding_payments {
                payments::handle_full_payment(&pool, contract_id, client_id)
                    .await
                    .map_err(|e| {
                        AppError::InternalServerError(format!(
                            "Failed to handle full payment: {:?}",
                            e
                        ))
                    })?;
                return Ok((StatusCode::OK, "Payment successful".to_string()));
            }

            Ok((StatusCode::OK, "Payment successful".to_string()))
        }
        PaymentRequest::SinglePayment(single_payment) => {
            if BigDecimal::from_f64(single_payment.amount)
                .expect("Failed to convert the payment amount into bigdecimal")
                != contract.price
            {
                return Err(AppError::BadRequest(
                    "Amount does not match contract price".to_string(),
                ));
            }

            pay_for_contract(&pool, contract_id, &client_id, single_payment.amount)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to pay for contract: {:?}", e))
                })?;

            payments::handle_full_payment(&pool, contract_id, client_id)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to handle full payment: {:?}", e))
                })?;

            Ok((StatusCode::OK, "Payment successful".to_string()))
        }
    }
}
