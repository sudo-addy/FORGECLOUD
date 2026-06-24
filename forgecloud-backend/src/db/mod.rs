use sqlx::{postgres::{PgConnectOptions, PgPoolOptions}, PgPool};
use std::str::FromStr;

pub async fn init_db(database_url: &str) -> Result<PgPool, anyhow::Error> {
    let connection_options = PgConnectOptions::from_str(database_url)?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(connection_options)
        .await?;
    
    Ok(pool)
}
