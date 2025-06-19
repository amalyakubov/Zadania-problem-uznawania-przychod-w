use crate::client::ClientId;
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

// TODO: Prepare migrations for this
pub async fn initialize_db(pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;

    sqlx::query!(
        "
        CREATE TABLE IF NOT EXISTS personal_client (
            first_name TEXT,    
            last_name TEXT,
            email TEXT,
            phone_number TEXT,
            pesel VARCHAR(11) PRIMARY KEY NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            is_deleted BOOLEAN NOT NULL DEFAULT FALSE
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "
        CREATE TABLE IF NOT EXISTS company_client (
            name TEXT NOT NULL,
            address TEXT NOT NULL,
            email TEXT NOT NULL,
            phone_number TEXT NOT NULL,
            krs VARCHAR(10) PRIMARY KEY NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            is_deleted BOOLEAN NOT NULL DEFAULT FALSE
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "
        CREATE TABLE IF NOT EXISTS software (
        id SERIAL PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT NOT NULL,
        version TEXT NOT NULL,
        category TEXT NOT NULL,
        price NUMERIC(10, 2) NOT NULL,
        is_deleted BOOLEAN NOT NULL DEFAULT FALSE
    )"
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "
        CREATE TABLE IF NOT EXISTS discount (
        id SERIAL PRIMARY KEY,
        name TEXT NOT NULL,
        discounted_products INTEGER REFERENCES software(id),
        percentage NUMERIC(7, 5) NOT NULL,
        start_date DATE NOT NULL,
        end_date DATE NOT NULL,
        is_signed BOOLEAN NOT NULL DEFAULT FALSE,
        is_deleted BOOLEAN NOT NULL DEFAULT FALSE
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "
        CREATE TABLE IF NOT EXISTS private_contract (
        id SERIAL PRIMARY KEY,
        client_id VARCHAR(11) REFERENCES personal_client(pesel),
        product_id INTEGER REFERENCES software(id),
        start_date DATE NOT NULL,
        end_date DATE NOT NULL,
        is_signed BOOLEAN NOT NULL DEFAULT FALSE,
        is_deleted BOOLEAN NOT NULL DEFAULT FALSE
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "
        CREATE TABLE IF NOT EXISTS corporate_contract (
        id SERIAL PRIMARY KEY,
        client_id VARCHAR(10) REFERENCES company_client(krs),
        product_id INTEGER REFERENCES software(id),
        start_date DATE NOT NULL,
        end_date DATE NOT NULL,
        is_signed BOOLEAN NOT NULL DEFAULT FALSE,
        is_deleted BOOLEAN NOT NULL DEFAULT FALSE
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(())
}

pub async fn check_if_product_exists(
    pool: &Pool<Postgres>,
    product_id: u32,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT 1 FROM software WHERE id = $1", product_id)
        .fetch_optional(pool)
        .await?;
    match result {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

pub async fn check_if_client_exists(
    pool: &Pool<Postgres>,
    client_id: ClientId,
    client_type: &str, // "personal" or "company"
) -> Result<bool, sqlx::Error> {
    let result = match client_type {
        "personal" => {
            sqlx::query!(
                "SELECT 1 FROM personal_client WHERE pesel = $1 AND is_deleted = FALSE",
                client_id.0
            )
            .fetch_optional(pool)
            .await?
        }
        "company" => {
            sqlx::query!(
                "SELECT 1 FROM company_client WHERE krs = $1 AND is_deleted = FALSE",
                client_id.0
            )
            .fetch_optional(pool)
            .await?
        }
        _ => return Ok(false),
    };

    match result {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

pub async fn check_product_and_client_exist(
    pool: &Pool<Postgres>,
    product_id: u32,
    client_id: ClientId,
    client_type: &str,
) -> Result<(bool, bool), sqlx::Error> {
    let (product_exists, client_exists) = tokio::join!(
        check_if_product_exists(pool, product_id),
        check_if_client_exists(pool, client_id, client_type)
    );

    Ok((product_exists?, client_exists?))
}

pub async fn find_discounts_for_client(
    pool: &Pool<Postgres>,
    product_id: u32,
    client_id: ClientId,
) -> Result<Option<f64>, sqlx::Error> {
    let highest_discount = sqlx::query!(
        "SELECT percentage FROM discount WHERE discounted_products = $1 AND is_deleted = FALSE AND start_date <= CURRENT_DATE AND end_date > CURRENT_DATE ORDER BY percentage DESC LIMIT 1",
        product_id
    )
    .fetch_optional(pool)
    .await?;
    match highest_discount {
        Some(discount) => Ok(Some(discount.percentage)),
        None => Ok(None),
    }

    let mut additional_discount = None;
    match client_id {
        // handle recurring clients
        ClientId::Individual(pesel) => {
            let result = sqlx::query!("
                SELECT COUNT(*) FROM private_contract WHERE client_id = $1 AND is_deleted = FALSE AND start_date <= CURRENT_DATE AND end_date > CURRENT_DATE ORDER BY start_date DESC LIMIT 1
            ", pesel)
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
            let result = sqlx::query!("
                SELECT COUNT(*) FROM corporate_contract WHERE client_id = $1 AND is_deleted = FALSE AND start_date <= CURRENT_DATE AND end_date > CURRENT_DATE ORDER BY start_date DESC LIMIT 1
            ", krs)
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
    Ok(final_discount)
}

pub async fn get_price_for_product(
    pool: &Pool<Postgres>,
    product_id: u32,
) -> Result<f64, (sqlx::Error, String)> {
    let result = sqlx::query!("SELECT price FROM software WHERE id = $1", product_id)
        .fetch_optional(pool)
        .await?;
    match result {
        Some(price) => Ok(price.price),
        None => Err((
            sqlx::Error::RowNotFound,
            "Failed to determine the price of the product".to_string(),
        )),
    }
}

pub async fn create_contract_in_db(
    pool: &Pool<Postgres>,
    price: f64,
    product_id: u32,
    client_id: ClientId,
    client_type: &str,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    match client_type {
        "personal" => {
            sqlx::query!("INSERT INTO private_contract (client_id, product_id, start_date, end_date, is_signed, is_deleted) VALUES ($1, $2, $3, $4, $5, $6)", client_id, product_id, start_date, end_date, false, false)
                .execute(pool)
                .await?;
            Ok(())
        }
        "company" => {
            sqlx::query!("INSERT INTO corporate_contract (client_id, product_id, start_date, end_date, is_signed, is_deleted) VALUES ($1, $2, $3, $4, $5, $6)", client_id, product_id, start_date, end_date, false, false)
                .execute(pool)
                .await?;
            Ok(())
        }
        _ => return Err(sqlx::Error::RowNotFound),
    }
}
