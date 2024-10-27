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
        repository)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        metadata.name.original_str(),
        metadata.description.as_ref(),
        metadata.documentation,
        metadata.homepage,
        metadata.readme,
        metadata.readme_file,
        metadata.license,
        metadata.license_file,
        metadata.repository,
    ).execute(exec).await?;
    Ok(())
}
pub async fn add_keywords(metadata: &Metadata, exec: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query!("INSERT INTO keywords (crate_id, keyword)
        VALUES ((SELECT crate_id FROM crates WHERE original_name = $1), unnest($2::TEXT[]))",
        metadata.name.original_str(),
        &metadata.keywords.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
    ).execute(&mut *exec).await?;
    Ok(())
}
pub async fn delete_keywords(crate_name: &CrateName, exec: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM keywords
        WHERE crate_id
        IN (SELECT crate_id FROM crates WHERE original_name = $1)",
        crate_name.original_str()
    )
        .execute(exec)
        .await?;
    Ok(())
}
// pub async fn insert_categories(metadata: &Metadata, exec: &mut PgConnection) -> Result<Vec<String>, sqlx::Error> {
//     sqlx::query!("
//     WITH valid_inserts AS (
//         INSERT INTO crate_categories (crate_id, category_id)
//         SELECT crates.crate_id, valid_categories.category_id
//         FROM unnest($1::TEXT[]) AS category
//         JOIN valid_categories ON valid_categories.category_name = category
//         JOIN crates ON crates.original_name = $2
//         RETURNING valid_categories.category_name
//     )
//     SELECT category
//     FROM unnest($1::TEXT[]) AS category
//     LEFT JOIN valid_inserts ON valid_inserts.category_name = category
//     WHERE valid_inserts.category_name IS NULL;",
//     metadata.categories.iter().map(|x| x.to_string()).collect::<Vec<()>>(),
//     metadata.name.original_str())
// }
#[derive(Clone, Copy, Debug)]
pub enum CrateExists {
    /// Crate matches exactly with name in database
    Yes,
    /// Crate matches, but is capitalized differently or switches -/_
    NoButNormalized,
    /// Crate doesn't exist in database
    No
}
