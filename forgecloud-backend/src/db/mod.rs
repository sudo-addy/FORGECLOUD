use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn init_db(database_url: &str) -> Result<PgPool, anyhow::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    
    Ok(pool)
}
