use sqlx::{Pool, Postgres};

use crate::crate_name::CrateName;

pub async fn crate_exists_exact(crate_name: &CrateName, pool: &Pool<Postgres>) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!("SELECT EXISTS(SELECT crate_id, original_name FROM crates WHERE original_name = $1)", crate_name.original_str())
        .fetch_one(pool)
        .await?;
    Ok(res.exists.unwrap())
}
pub async fn crate_exists_or_normalized(crate_name: &CrateName, pool: &Pool<Postgres>) -> Result<CrateExists, sqlx::Error> {
    if crate_exists_exact(crate_name, pool).await? {
        return Ok(CrateExists::Yes)
    }
    let res_normalized = sqlx::query!("SELECT EXISTS(SELECT crate_id, original_name FROM crates
        WHERE normalize_crate_name(original_name) = $1)", crate_name.normalized())
        .fetch_one(pool)
        .await?;
    if res_normalized.exists.unwrap() {
        Ok(CrateExists::NoButNormalized)
    } else {
        Ok(CrateExists::No)
    }
}
#[derive(Clone, Copy, Debug)]
pub enum CrateExists {
    Yes,
    NoButNormalized,
    No
}
