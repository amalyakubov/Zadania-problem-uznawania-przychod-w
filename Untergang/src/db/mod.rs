use sqlx::{Pool, Postgres};

pub async fn connect_db() -> Result<Pool<Postgres>, sqlx::Error> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool: Pool<Postgres> = match Pool::connect(&db_url).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("Error connecting to the database: {}", e);
            std::process::exit(1);
        }
    };
    Ok(pool)
}

pub async fn initialize_db(pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS personal_clients (
            id SERIAL PRIMARY KEY,
            first_name VARCHAR(255) NOT NULL,
            last_name VARCHAR(255) NOT NULL,
            email VARCHAR(255) NOT NULL,
            phone_number VARCHAR(255) NOT NULL,
            pesel VARCHAR(11) NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS company_clients (
            id SERIAL PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            address VARCHAR(255) NOT NULL,
            email VARCHAR(255) NOT NULL,
            phone_number VARCHAR(255) NOT NULL,
            krs VARCHAR(10) NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(())
}
