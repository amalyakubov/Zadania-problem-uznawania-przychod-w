use crate::client::{ClientId, Contract};
use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};

pub async fn connect_db() -> Result<Pool<Postgres>, sqlx::Error> {
    // For dev use only:
    // let db_url = "postgres://postgres:password@localhost:5432/Untergang";
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
    let result = sqlx::query_scalar::<_, bool>("SELECT 1 as found FROM software WHERE id = $1")
        .bind(*product_id)
        .fetch_optional(pool)
        .await?;
    match result {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

pub async fn check_if_client_exists(
    pool: &Pool<Postgres>,
    client_id: &ClientId,
) -> Result<bool, sqlx::Error> {
    match client_id {
        ClientId::Individual(pesel) => {
            let result = sqlx::query_scalar::<_, bool>(
                "SELECT 1 as found FROM personal_client WHERE pesel = $1 AND is_deleted = FALSE",
            )
            .bind(pesel)
            .fetch_optional(pool)
            .await?;
            Ok(result.is_some())
        }
        ClientId::Company(krs) => {
            let result = sqlx::query_scalar::<_, bool>(
                "SELECT 1 as found FROM company_client WHERE krs = $1 AND is_deleted = FALSE",
            )
            .bind(krs)
            .fetch_optional(pool)
            .await?;
            Ok(result.is_some())
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

    match highest_discount {
        Some(discount) => Ok(discount),
        None => Err(sqlx::Error::RowNotFound),
    };

    let mut additional_discount = None;
    match client_id {
        // handle recurring clients
        ClientId::Individual(pesel) => {
            let result = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM private_contract WHERE client_id = $1 AND is_deleted = FALSE AND start_date <= CURRENT_DATE AND end_date > CURRENT_DATE",
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
                "SELECT COUNT(*) FROM corporate_contract WHERE client_id = $1 AND is_deleted = FALSE AND start_date <= CURRENT_DATE AND end_date > CURRENT_DATE",
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
    let result = sqlx::query_scalar::<_, f64>("SELECT price FROM software WHERE id = $1")
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
        Some(price) => Ok(price),
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

    match client_id {
        ClientId::Individual(pesel) => {
            sqlx::query!(
                "INSERT INTO private_contract (client_id, product_id, price, start_date, end_date, years_supported, is_signed, is_deleted) 
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)", 
                pesel, product_id, price_decimal, start_date.naive_utc(), end_date.naive_utc(), years_supported, false, false
            )
            .execute(pool)
            .await?;
        }
        ClientId::Company(krs) => {
            sqlx::query!(
                "INSERT INTO corporate_contract (client_id, product_id, price, start_date, end_date, years_supported, is_signed, is_deleted) 
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)", 
                krs, product_id, price_decimal, start_date.naive_utc(), end_date.naive_utc(), years_supported, false, false
            )
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

pub async fn check_if_client_has_contract_for_product(
    pool: &Pool<Postgres>,
    client_id: ClientId,
    product_id: i32,
) -> Result<bool, sqlx::Error> {
    match client_id {
        ClientId::Individual(pesel) => {
            let result = sqlx::query_scalar::<_, bool>(
                "SELECT 1 as found FROM private_contract WHERE client_id = $1 AND product_id = $2 AND is_deleted = FALSE",
            )
            .bind(pesel)
            .bind(product_id)
            .fetch_optional(pool)
            .await?;

            match result {
                Some(_) => Ok(true),
                None => Ok(false),
            }
        }
        ClientId::Company(krs) => {
            let result = sqlx::query_scalar::<_, bool>(
                "SELECT 1 as found FROM corporate_contract WHERE client_id = $1 AND product_id = $2 AND is_deleted = FALSE",
            )
            .bind(krs)
            .bind(product_id)
            .fetch_optional(pool)
            .await?;
            match result {
                Some(_) => Ok(true),
                None => Ok(false),
            }
        }
    }
}

pub async fn get_contract_by_id(
    pool: &Pool<Postgres>,
    client_id: ClientId,
    contract_id: i32,
) -> Result<Contract, sqlx::Error> {
    match client_id {
        ClientId::Individual(pesel) => {
            let result = sqlx::query!(
                "SELECT id, price, product_id, client_id, start_date, end_date, years_supported FROM private_contract WHERE id = $1 AND client_id = $2 AND is_deleted = FALSE",
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
                }),
                None => Err(sqlx::Error::RowNotFound),
            }
        }
        ClientId::Company(krs) => {
            let result = sqlx::query!(
                "SELECT id, price, product_id, client_id, start_date, end_date, years_supported FROM corporate_contract WHERE id = $1 AND client_id = $2 AND is_deleted = FALSE",
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
                }),
                None => Err(sqlx::Error::RowNotFound),
            }
        }
    }
}

pub async fn pay_for_contract(
    pool: Pool<Postgres>,
    contract_id: i32,
    client_id: ClientId,
) -> Result<(), sqlx::Error> {
    match client_id {
        ClientId::Individual(pesel) => {
            match sqlx::query!(
                "UPDATE private_contract SET is_paid = TRUE WHERE id = $1 AND client_id = $2",
                contract_id,
                pesel
            )
            .execute(&pool)
            .await
            {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        }
        ClientId::Company(krs) => {
            match sqlx::query!(
                "UPDATE corporate_contract SET is_paid = TRUE WHERE id = $1 AND client_id = $2",
                contract_id,
                krs
            )
            .execute(&pool)
            .await
            {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        }
    }
}
