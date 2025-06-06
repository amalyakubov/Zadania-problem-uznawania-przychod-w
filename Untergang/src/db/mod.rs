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
        CREATE TABLE IF NOT EXISTS personal_clients (
            id SERIAL PRIMARY KEY,
            first_name TEXT,    
            last_name TEXT,
            email TEXT,
            phone_number TEXT,
            pesel VARCHAR(11),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            is_deleted BOOLEAN NOT NULL DEFAULT FALSE
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "
        CREATE TABLE IF NOT EXISTS company_clients (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            address TEXT NOT NULL,
            email TEXT NOT NULL,
            phone_number TEXT NOT NULL,
            krs VARCHAR(10) NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            is_deleted BOOLEAN NOT NULL DEFAULT FALSE
        )
        ",
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "CREATE TABLE IF NOT EXISTS software (
        id SERIAL PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT NOT NULL,
        version TEXT NOT NULL,
        created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
        is_deleted BOOLEAN NOT NULL DEFAULT FALSE
    )"
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(())
}
