use sqlx::{Executor, PgConnection, Pool, Postgres};

use crate::{crate_name::CrateName, publish::Metadata};

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
pub async fn add_crate(metadata: &Metadata, exec: impl Executor<'_, Database = Postgres>) -> Result<(), sqlx::Error> {
    sqlx::query!("INSERT INTO crates (
        original_name, description,
        documentation, homepage,
        readme, readme_file,
        license, license_file,
        repository, links, rust_version)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        metadata.name.original_str(),
        metadata.description.as_ref(),
        metadata.documentation,
        metadata.homepage,
        metadata.readme,
        metadata.readme_file,
        metadata.license,
        metadata.license_file,
        metadata.repository,
        metadata.links,
        metadata.rust_version
    ).execute(exec).await?;
    Ok(())
}
pub async fn add_keywords(metadata: &Metadata, exec: &mut PgConnection) -> Result<(), sqlx::Error> {
    for keyword in &metadata.keywords {
        sqlx::query!("INSERT INTO keywords (crate_id, keyword)
            VALUES ((SELECT crate_id FROM crates WHERE original_name = $1), $2)",
            metadata.name.original_str(),
            keyword.as_ref()
        ).execute(&mut *exec).await?;
    }
    Ok(())
}
#[derive(Clone, Copy, Debug)]
pub enum CrateExists {
    /// Crate matches exactly with name in database
    Yes,
    /// Crate matches, but is capitalized differently or switches -/_
    NoButNormalized,
    /// Crate doesn't exist in database
    No
}
