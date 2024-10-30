use std::collections::HashSet;

use sqlx::{Executor, PgConnection, Postgres};

use crate::{crate_name::CrateName, publish::Metadata};

pub async fn crate_exists_exact(
    crate_name: &CrateName,
    exec: &mut PgConnection,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!(
        "SELECT EXISTS(SELECT crate_id, original_name FROM crates WHERE original_name = $1)",
        crate_name.original_str()
    )
    .fetch_one(exec)
    .await?;
    Ok(res.exists.unwrap())
}
pub async fn crate_exists_or_normalized(
    crate_name: &CrateName,
    exec: &mut PgConnection,
) -> Result<CrateExists, sqlx::Error> {
    if crate_exists_exact(crate_name, &mut *exec).await? {
        return Ok(CrateExists::Yes);
    }
    let res_normalized = sqlx::query!(
        "SELECT EXISTS(SELECT crate_id, original_name FROM crates
        WHERE normalize_crate_name(original_name) = $1)",
        crate_name.normalized()
    )
    .fetch_one(&mut *exec)
    .await?;
    if res_normalized.exists.unwrap() {
        Ok(CrateExists::NoButNormalized)
    } else {
        Ok(CrateExists::No)
    }
}
pub async fn add_crate(
    metadata: &Metadata,
    exec: impl Executor<'_, Database = Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO crates (
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
    )
    .execute(exec)
    .await?;
    Ok(())
}
pub async fn add_keywords(metadata: &Metadata, exec: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO keywords (crate_id, keyword)
        VALUES ((SELECT crate_id FROM crates WHERE original_name = $1), unnest($2::TEXT[]))",
        metadata.name.original_str(),
        &metadata
            .keywords
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>(),
    )
    .execute(&mut *exec)
    .await?;
    Ok(())
}
pub async fn delete_keywords(
    crate_name: &CrateName,
    exec: &mut PgConnection,
) -> Result<(), sqlx::Error> {
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
pub async fn insert_categories(
    categories: HashSet<String>,
    crate_name: &CrateName,
    exec: &mut PgConnection,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO crate_categories (crate_id, category_id)
        SELECT crates.crate_id, valid_categories.category_id
        FROM crates
        JOIN valid_categories ON valid_categories.category_name = ANY($1::TEXT[])
        WHERE crates.original_name = $2",
        &categories.into_iter().collect::<Vec<_>>(),
        crate_name.original_str()
    )
    .execute(&mut *exec)
    .await?;
    Ok(())
}
pub async fn get_bad_categories(
    metadata: &Metadata,
    exec: &mut PgConnection,
) -> Result<HashSet<String>, sqlx::Error> {
    sqlx::query!(
        "SELECT category
        FROM unnest($1::TEXT[]) AS category
        LEFT JOIN valid_categories ON valid_categories.category_name = category
        WHERE valid_categories.category_name IS NULL",
        &metadata.categories.iter().cloned().collect::<Vec<_>>()
    )
    .fetch_all(exec)
    .await
    .map(|records| {
        records
            .into_iter()
            .map(|x| x.category.expect("should not come out NULL"))
            .collect()
    })
}
pub async fn delete_category_entries(
    crate_name: &CrateName,
    exec: &mut PgConnection,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM
        crate_categories
        WHERE crate_id
        IN (SELECT crate_id FROM crates WHERE original_name = $1)",
        crate_name.original_str()
    )
    .execute(exec)
    .await?;
    Ok(())
}
pub async fn add_version(
    metadata: &Metadata,
    cksum: &str,
    exec: &mut PgConnection
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO versions (crate, vers, cksum, links, rust_version)
        SELECT crates.crate_id, $1, $2, $3, $4
        FROM crates
        WHERE crates.original_name = $5",
        metadata.vers.to_string(),
        cksum,
        metadata.links,
        metadata.rust_version.as_ref().map(|rv| rv.to_string()),
        metadata.name.original_str()
    )
    .execute(&mut *exec)
    .await?;
    // features2 is empty
    for (feature, feature_deps) in &metadata.features {
        sqlx::query!(
            "INSERT INTO version_features (crate_id, crate_version, feature_name)
            SELECT crates.crate_id, $1, $2
            FROM crates
            WHERE crates.original_name = $3",
            metadata.vers.to_string(),
            feature.as_ref(),
            metadata.name.original_str()
        )
        .execute(&mut *exec)
        .await?;
        for dependency_name in feature_deps {
            sqlx::query!(
                "INSERT INTO feature_dependencies (crate_id, crate_version, feature_name, dependency_name)
                SELECT crates.crate_id, $1, $2, $3
                FROM crates
                WHERE crates.original_name = $4",
                metadata.vers.to_string(),
                feature.as_ref(),
                dependency_name,
                metadata.name.original_str(),
            )
            .execute(&mut *exec)
            .await?;
        }
    }
    for author in &metadata.authors {
        sqlx::query!(
            "INSERT INTO version_authors (crate_id, version, author)
            SELECT crates.crate_id, $1, $2
            FROM crates
            WHERE crates.original_name = $3",
            metadata.vers.to_string(),
            author,
            metadata.name.original_str(),
        )
        .execute(&mut *exec)
        .await?;
    }
    Ok(())
}
pub async fn get_versions(crate_name: &CrateName, exec: &mut PgConnection) -> Result<Vec<semver::Version>, sqlx::Error> {
    Ok(sqlx::query!(
        "SELECT vers
        FROM versions
        JOIN crates
        ON versions.crate = crates.crate_id
        WHERE crates.original_name = $1",
        crate_name.original_str()
    )
    .fetch_all(exec)
    .await?
    .into_iter()
    .map(|x| x.vers.parse().expect("hope all the database contents are valid"))
    .collect())
}

#[derive(Clone, Copy, Debug)]
pub enum CrateExists {
    /// Crate matches exactly with name in database
    Yes,
    /// Crate matches, but is capitalized differently or switches -/_
    NoButNormalized,
    /// Crate doesn't exist in database
    No,
}
