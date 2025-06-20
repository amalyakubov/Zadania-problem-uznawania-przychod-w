use crate::client::{ClientId, Contract, Payment};
use crate::handler::AppError;
use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};

pub async fn connect_db() -> Result<Pool<Postgres>, sqlx::Error> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool: Pool<Postgres> = match Pool::connect(&db_url).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("Error connecting to the database: {}", e);
            return Err(e);
        }
    };
    Ok(pool)
}

pub async fn check_if_product_exists(
    pool: &Pool<Postgres>,
    product_id: &i32,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM software WHERE id = $1)")
            .bind(*product_id)
            .fetch_one(pool)
            .await?;
    Ok(result)
}

pub async fn check_if_client_exists(
    pool: &Pool<Postgres>,
    client_id: &ClientId,
) -> Result<bool, sqlx::Error> {
    match client_id {
        ClientId::Individual(pesel) => {
            let result = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM personal_client WHERE pesel = $1 AND is_deleted = FALSE)",
            )
            .bind(pesel)
            .fetch_one(pool)
            .await?;
            Ok(result)
        }
        ClientId::Company(krs) => {
            let result = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM company_client WHERE krs = $1 AND is_deleted = FALSE)",
            )
            .bind(krs)
            .fetch_one(pool)
            .await?;
            Ok(result)
        }
    }
}

pub async fn check_product_and_client_exist(
    pool: &Pool<Postgres>,
    product_id: i32,
    client_id: ClientId,
) -> Result<(bool, bool), sqlx::Error> {
    let (product_exists, client_exists) = tokio::join!(
        check_if_product_exists(pool, &product_id),
        check_if_client_exists(pool, &client_id)
    );

    Ok((product_exists?, client_exists?))
}

pub async fn find_discounts_for_client(
    pool: &Pool<Postgres>,
    product_id: i32,
    client_id: ClientId,
) -> Result<Option<f64>, sqlx::Error> {
    let highest_discount = sqlx::query_scalar::<_, f64>(
        "SELECT percentage FROM discount WHERE discounted_products = $1 AND is_deleted = FALSE AND start_date <= CURRENT_DATE AND end_date > CURRENT_DATE ORDER BY percentage DESC LIMIT 1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await?;

    let mut additional_discount = None;
    match client_id {
        // handle recurring clients
        ClientId::Individual(pesel) => {
            let result = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM contract WHERE client_id = $1 AND is_deleted = FALSE AND start_date <= CURRENT_DATE AND end_date > CURRENT_DATE",
            )
            .bind(pesel)
            .fetch_optional(pool)
            .await?;
            match result {
                Some(count) => {
                    if count >= 1 {
                        additional_discount = Some(0.05);
                    }
                }
                None => {
                    additional_discount = None;
                }
            }
        }
        ClientId::Company(krs) => {
            let result = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM contract WHERE client_id = $1 AND is_deleted = FALSE AND start_date <= CURRENT_DATE AND end_date > CURRENT_DATE",
            )
            .bind(krs)
            .fetch_optional(pool)
            .await?;
            match result {
                Some(count) => {
                    if count >= 1 {
                        additional_discount = Some(0.05);
                    }
                }
                None => {
                    additional_discount = None;
                }
            }
        }
    }

    let final_discount = match highest_discount {
        Some(discount) => match additional_discount {
            Some(additional) => discount + additional,
            None => discount,
        },
        None => match additional_discount {
            Some(additional) => additional,
            None => 0.0,
        },
    };
    Ok(Some(final_discount))
}

pub async fn get_price_for_product(
    pool: &Pool<Postgres>,
    product_id: i32,
) -> Result<f64, (sqlx::Error, String)> {
    let result = sqlx::query_scalar::<_, BigDecimal>("SELECT price FROM software WHERE id = $1")
        .bind(product_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            (
                e,
                "Failed to determine the price of the product".to_string(),
            )
        })?;
    match result {
        Some(price) => {
            // Convert BigDecimal to f64
            price.to_f64().ok_or((
                sqlx::Error::Decode("Failed to convert price to f64".into()),
                "Failed to convert price to f64".to_string(),
            ))
        }
        None => Err((
            sqlx::Error::RowNotFound,
            "Failed to determine the price of the product".to_string(),
        )),
    }
}

pub async fn create_contract_in_db(
    pool: &Pool<Postgres>,
    price: f64,
    product_id: i32,
    client_id: ClientId,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    years_supported: i32,
) -> Result<(), sqlx::Error> {
    let price_decimal = BigDecimal::from_f64(price)
        .ok_or(sqlx::Error::Configuration("Invalid price format".into()))?;

    let (contract_type, personal_client_pesel, company_client_krs) = match client_id {
        ClientId::Individual(pesel) => ("private", Some(pesel), None),
        ClientId::Company(krs) => ("corporate", None, Some(krs)),
    };

    sqlx::query!(
        "INSERT INTO contract (contract_type, personal_client_pesel, company_client_krs, product_id, price, start_date, end_date, years_supported, is_signed, is_deleted) 
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)", 
        contract_type, personal_client_pesel, company_client_krs, product_id, price_decimal, start_date.naive_utc(), end_date.naive_utc(), years_supported, false, false
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn check_if_client_has_contract_for_product(
    pool: &Pool<Postgres>,
    client_id: ClientId,
    product_id: i32,
) -> Result<bool, sqlx::Error> {
    let result = match client_id {
        ClientId::Individual(pesel) => {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM contract WHERE personal_client_pesel = $1 AND product_id = $2 AND is_deleted = FALSE)",
            )
            .bind(pesel)
            .bind(product_id)
            .fetch_one(pool)
            .await?
        }
        ClientId::Company(krs) => {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM contract WHERE company_client_krs = $1 AND product_id = $2 AND is_deleted = FALSE)",
            )
            .bind(krs)
            .bind(product_id)
            .fetch_one(pool)
            .await?
        }
    };
    Ok(result)
}

pub async fn get_contract_by_id(
    pool: &Pool<Postgres>,
    client_id: ClientId,
    contract_id: i32,
) -> Result<Contract, sqlx::Error> {
    match client_id {
        ClientId::Individual(pesel) => {
            let result = sqlx::query!(
                "SELECT id, price, product_id, start_date, end_date, years_supported, is_signed, is_paid, is_deleted 
                 FROM contract 
                 WHERE id = $1 AND personal_client_pesel = $2 AND is_deleted = FALSE",
                contract_id,
                pesel,
            )
            .fetch_optional(pool)
            .await?;

            match result {
                Some(contract) => Ok(Contract {
                    id: contract.id,
                    price: contract.price,
                    product_id: contract
                        .product_id
                        .expect("Product ID not found on the contract"),
                    client_id: ClientId::Individual(pesel),
                    start_date: DateTime::from_naive_utc_and_offset(contract.start_date, Utc),
                    end_date: DateTime::from_naive_utc_and_offset(contract.end_date, Utc),
                    years_supported: contract.years_supported,
                    is_signed: contract.is_signed,
                    is_paid: contract.is_paid,
                    is_deleted: contract.is_deleted,
                }),
                None => Err(sqlx::Error::RowNotFound),
            }
        }
        ClientId::Company(krs) => {
            let result = sqlx::query!(
                "SELECT id, price, product_id, start_date, end_date, years_supported, is_signed, is_paid, is_deleted 
                 FROM contract 
                 WHERE id = $1 AND company_client_krs = $2 AND is_deleted = FALSE",
                contract_id,
                krs,
            )
            .fetch_optional(pool)
            .await?;

            match result {
                Some(contract) => Ok(Contract {
                    id: contract.id,
                    price: contract.price,
                    product_id: contract
                        .product_id
                        .expect("Product ID not found on the contract"),
                    client_id: ClientId::Company(krs),
                    start_date: DateTime::from_naive_utc_and_offset(contract.start_date, Utc),
                    end_date: DateTime::from_naive_utc_and_offset(contract.end_date, Utc),
                    years_supported: contract.years_supported,
                    is_signed: contract.is_signed,
                    is_paid: contract.is_paid,
                    is_deleted: contract.is_deleted,
                }),
                None => Err(sqlx::Error::RowNotFound),
            }
        }
    }
}

pub async fn pay_for_contract(
    pool: &Pool<Postgres>,
    contract_id: i32,
    _client_id: &ClientId,
    amount: f64,
) -> Result<(), AppError> {
    match payments::create_payment_record_in_db(pool, contract_id, amount)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to create payment: {:?}", e)))
    {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub async fn get_payments_for_contract(
    pool: &Pool<Postgres>,
    contract_id: i32,
) -> Result<Vec<Payment>, AppError> {
    let result = sqlx::query!(
        "SELECT id, contract_id, amount, payment_date, is_deleted FROM payment WHERE contract_id = $1",
        contract_id
    )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            AppError::InternalServerError(format!("Failed to get payments: {}", e))
        })?;

    Ok(result
        .into_iter()
        .map(|p| Payment {
            id: p.id,
            contract_id: p.contract_id.expect("Contract ID not found on the payment"),
            amount: p.amount,
            payment_date: DateTime::from_naive_utc_and_offset(p.payment_date, Utc),
            is_deleted: p.is_deleted,
        })
        .collect())
}

pub mod payments {
    use super::*;
    use crate::db::get_payments_for_contract;
    use bigdecimal::ToPrimitive;

    pub async fn check_outstanding_payments(
        pool: &Pool<Postgres>,
        contract_id: i32,
    ) -> Result<f64, AppError> {
        let payments = get_payments_for_contract(pool, contract_id)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to get payments: {:?}", e))
            })?;

        let outstanding_payments = payments
            .iter()
            .map(|p| p.amount.to_f64().expect("Failed to convert amount to f64"))
            .sum();

        Ok(outstanding_payments)
    }

    pub async fn create_payment_record_in_db(
        pool: &Pool<Postgres>,
        contract_id: i32,
        amount: f64,
    ) -> Result<(), AppError> {
        let amount_decimal = BigDecimal::from_f64(amount).ok_or(AppError::InternalServerError(
            "Invalid amount format".to_string(),
        ))?;

        match sqlx::query!(
            "INSERT INTO payment (contract_id, amount) VALUES ($1, $2)",
            contract_id,
            amount_decimal
        )
        .execute(pool)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(AppError::InternalServerError(format!(
                "Failed to create payment: {:?}",
                e
            ))),
        }
    }

    pub async fn handle_full_payment(
        pool: &Pool<Postgres>,
        contract_id: i32,
        client_id: ClientId,
    ) -> Result<(), AppError> {
        match client_id {
            ClientId::Individual(pesel) => {
                match sqlx::query!(
                    "UPDATE contract SET is_paid = TRUE WHERE id = $1 AND personal_client_pesel = $2",
                    contract_id,
                    pesel
                )
                .execute(pool)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to handle full payment: {:?}", e))
                }) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e),
                }
            }
            ClientId::Company(krs) => {
                match sqlx::query!(
                    "UPDATE contract SET is_paid = TRUE WHERE id = $1 AND company_client_krs = $2",
                    contract_id,
                    krs
                )
                .execute(pool)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to handle full payment: {:?}", e))
                }) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e),
                }
            }
        }
    }
}
