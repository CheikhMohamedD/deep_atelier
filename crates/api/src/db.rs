//! Accès à la base : projets, document courant, versions. Chaque requête est restreinte au
//! propriétaire (`owner_id`) ; une écriture verrouille d'abord la ligne du projet, ce qui
//! sérialise les écritures d'un même projet (numéros de version, concurrence optimiste).

use serde::Serialize;
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::document::Checked;
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    /// Version du document courant.
    pub document_version: i64,
}

/// Document courant d'un projet.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DocumentState {
    pub version: i64,
    pub ir_version: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub document: Json<Value>,
}

/// Résultat d'une sauvegarde.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Saved {
    pub version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// Version d'un projet, sans son document.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Version {
    pub number: i32,
    pub name: Option<String>,
    pub message: Option<String>,
    pub origin: String,
    pub restored_from: Option<i32>,
    pub document_version: i64,
    pub ir_version: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub created_by: Option<Uuid>,
}

/// Version et son document.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct VersionWithDocument {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub version: Version,
    pub document: Json<Value>,
}

/// Début des requêtes de projet (colonnes de [`Project`]) ; requêtes SQL statiques, assemblées
/// par `concat!`.
macro_rules! select_project {
    () => {
        "select p.id, p.name, p.created_at, p.updated_at, d.version as document_version \
         from public.projects p join public.project_documents d on d.project_id = p.id "
    };
}

/// Colonnes de [`Version`].
macro_rules! version_columns {
    () => {
        "number, name, message, origin, restored_from, document_version, ir_version, created_at, created_by"
    };
}

pub async fn list_projects(pool: &PgPool, owner: Uuid) -> Result<Vec<Project>, ApiError> {
    let sql = concat!(
        select_project!(),
        "where p.owner_id = $1 order by p.updated_at desc, p.id"
    );
    Ok(sqlx::query_as(sql).bind(owner).fetch_all(pool).await?)
}

pub async fn get_project(pool: &PgPool, owner: Uuid, id: Uuid) -> Result<Project, ApiError> {
    sqlx::query_as(concat!(select_project!(), "where p.owner_id = $1 and p.id = $2"))
        .bind(owner)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Crée un projet et son document initial.
pub async fn create_project(
    pool: &PgPool,
    owner: Uuid,
    name: &str,
    document: &Checked,
) -> Result<(Project, DocumentState), ApiError> {
    let mut tx = pool.begin().await?;
    let id: Uuid = sqlx::query_scalar("insert into public.projects (owner_id, name) values ($1, $2) returning id")
        .bind(owner)
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
    let state: DocumentState = sqlx::query_as(
        "insert into public.project_documents (project_id, version, ir_version, document, updated_by) \
         values ($1, 1, $2, $3, $4) returning version, ir_version, updated_at, document",
    )
    .bind(id)
    .bind(document.ir_version)
    .bind(Json(&document.json))
    .bind(owner)
    .fetch_one(&mut *tx)
    .await?;
    let project = project_in(&mut tx, owner, id).await?;
    tx.commit().await?;
    Ok((project, state))
}

pub async fn rename_project(pool: &PgPool, owner: Uuid, id: Uuid, name: &str) -> Result<Project, ApiError> {
    let mut tx = pool.begin().await?;
    let renamed =
        sqlx::query("update public.projects set name = $3, updated_at = now() where owner_id = $1 and id = $2")
            .bind(owner)
            .bind(id)
            .bind(name)
            .execute(&mut *tx)
            .await?;
    if renamed.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    let project = project_in(&mut tx, owner, id).await?;
    tx.commit().await?;
    Ok(project)
}

/// Supprime un projet, son document et ses versions.
pub async fn delete_project(pool: &PgPool, owner: Uuid, id: Uuid) -> Result<(), ApiError> {
    let deleted = sqlx::query("delete from public.projects where owner_id = $1 and id = $2")
        .bind(owner)
        .bind(id)
        .execute(pool)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

pub async fn get_document(pool: &PgPool, owner: Uuid, id: Uuid) -> Result<DocumentState, ApiError> {
    sqlx::query_as(
        "select d.version, d.ir_version, d.updated_at, d.document from public.project_documents d \
         join public.projects p on p.id = d.project_id where p.owner_id = $1 and p.id = $2",
    )
    .bind(owner)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)
}

/// Remplace le document courant s'il est encore à la version `base_version` (concurrence
/// optimiste) ; sinon `VERSION_CONFLICT` avec la version courante.
pub async fn save_document(
    pool: &PgPool,
    owner: Uuid,
    id: Uuid,
    base_version: i64,
    document: &Checked,
) -> Result<Saved, ApiError> {
    let mut tx = pool.begin().await?;
    lock_project(&mut tx, owner, id).await?;
    let current = current_version(&mut tx, id).await?;
    if current != base_version {
        return Err(ApiError::VersionConflict {
            base_version,
            current_version: current,
        });
    }
    let saved = replace_document(&mut tx, owner, id, document).await?;
    tx.commit().await?;
    Ok(saved)
}

pub async fn list_versions(pool: &PgPool, owner: Uuid, id: Uuid) -> Result<Vec<Version>, ApiError> {
    let mut conn = pool.acquire().await?;
    ensure_owned(&mut conn, owner, id).await?;
    let sql = concat!(
        "select ",
        version_columns!(),
        " from public.document_versions where project_id = $1 order by number desc"
    );
    Ok(sqlx::query_as(sql).bind(id).fetch_all(&mut *conn).await?)
}

/// Enregistre le document courant comme nouvelle version.
pub async fn create_version(
    pool: &PgPool,
    owner: Uuid,
    id: Uuid,
    name: Option<&str>,
    message: &str,
) -> Result<Version, ApiError> {
    let mut tx = pool.begin().await?;
    lock_project(&mut tx, owner, id).await?;
    let version = snapshot(&mut tx, owner, id, Snapshot::User { name, message }).await?;
    tx.commit().await?;
    Ok(version)
}

pub async fn get_version(pool: &PgPool, owner: Uuid, id: Uuid, number: i32) -> Result<VersionWithDocument, ApiError> {
    let mut conn = pool.acquire().await?;
    ensure_owned(&mut conn, owner, id).await?;
    let sql = concat!(
        "select ",
        version_columns!(),
        ", document from public.document_versions where project_id = $1 and number = $2"
    );
    sqlx::query_as(sql)
        .bind(id)
        .bind(number)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Document d'une version, pour la restaurer (contrôlé et migré hors transaction).
pub async fn version_document(pool: &PgPool, owner: Uuid, id: Uuid, number: i32) -> Result<Value, ApiError> {
    Ok(get_version(pool, owner, id, number).await?.document.0)
}

/// Restaure une version : l'état courant est d'abord gardé comme version `restore`, puis le
/// document de la version (déjà contrôlé) devient le document courant.
pub async fn restore_version(
    pool: &PgPool,
    owner: Uuid,
    id: Uuid,
    number: i32,
    base_version: i64,
    document: &Checked,
) -> Result<(Version, DocumentState), ApiError> {
    let mut tx = pool.begin().await?;
    lock_project(&mut tx, owner, id).await?;
    let current = current_version(&mut tx, id).await?;
    if current != base_version {
        return Err(ApiError::VersionConflict {
            base_version,
            current_version: current,
        });
    }
    let exists: bool = sqlx::query_scalar(
        "select exists (select 1 from public.document_versions where project_id = $1 and number = $2)",
    )
    .bind(id)
    .bind(number)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    let backup = snapshot(&mut tx, owner, id, Snapshot::BeforeRestore { restored_from: number }).await?;
    replace_document(&mut tx, owner, id, document).await?;
    let state: DocumentState = sqlx::query_as(
        "select version, ir_version, updated_at, document from public.project_documents where project_id = $1",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((backup, state))
}

/// Projet de l'utilisateur, lu dans une transaction.
async fn project_in(conn: &mut PgConnection, owner: Uuid, id: Uuid) -> Result<Project, ApiError> {
    sqlx::query_as(concat!(select_project!(), "where p.owner_id = $1 and p.id = $2"))
        .bind(owner)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Verrouille la ligne du projet de l'utilisateur jusqu'à la fin de la transaction.
async fn lock_project(conn: &mut PgConnection, owner: Uuid, id: Uuid) -> Result<(), ApiError> {
    sqlx::query_scalar::<_, Uuid>("select id from public.projects where owner_id = $1 and id = $2 for update")
        .bind(owner)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .map(|_| ())
        .ok_or(ApiError::NotFound)
}

async fn ensure_owned(conn: &mut PgConnection, owner: Uuid, id: Uuid) -> Result<(), ApiError> {
    let owned: bool =
        sqlx::query_scalar("select exists (select 1 from public.projects where owner_id = $1 and id = $2)")
            .bind(owner)
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
    owned.then_some(()).ok_or(ApiError::NotFound)
}

async fn current_version(conn: &mut PgConnection, id: Uuid) -> Result<i64, ApiError> {
    Ok(
        sqlx::query_scalar("select version from public.project_documents where project_id = $1")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?,
    )
}

async fn replace_document(
    conn: &mut PgConnection,
    owner: Uuid,
    id: Uuid,
    document: &Checked,
) -> Result<Saved, ApiError> {
    let saved: Saved = sqlx::query_as(
        "update public.project_documents \
         set version = version + 1, ir_version = $2, document = $3, updated_at = now(), updated_by = $4 \
         where project_id = $1 returning version, updated_at",
    )
    .bind(id)
    .bind(document.ir_version)
    .bind(Json(&document.json))
    .bind(owner)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query("update public.projects set updated_at = $2 where id = $1")
        .bind(id)
        .bind(saved.updated_at)
        .execute(&mut *conn)
        .await?;
    Ok(saved)
}

enum Snapshot<'a> {
    User { name: Option<&'a str>, message: &'a str },
    BeforeRestore { restored_from: i32 },
}

/// Copie le document courant dans une nouvelle version, numérotée après la dernière.
async fn snapshot(conn: &mut PgConnection, owner: Uuid, id: Uuid, kind: Snapshot<'_>) -> Result<Version, ApiError> {
    let (name, message, origin, restored_from) = match kind {
        Snapshot::User { name, message } => (name, Some(message), "user", None),
        Snapshot::BeforeRestore { restored_from } => (None, None, "restore", Some(restored_from)),
    };
    let sql = concat!(
        "insert into public.document_versions \
         (project_id, number, name, message, origin, restored_from, document_version, ir_version, document, created_by) \
         select d.project_id, \
                coalesce((select max(v.number) from public.document_versions v where v.project_id = $1), 0) + 1, \
                $2, $3, $4, $5, d.version, d.ir_version, d.document, $6 \
         from public.project_documents d where d.project_id = $1 \
         returning ",
        version_columns!()
    );
    Ok(sqlx::query_as(sql)
        .bind(id)
        .bind(name)
        .bind(message)
        .bind(origin)
        .bind(restored_from)
        .bind(owner)
        .fetch_one(&mut *conn)
        .await?)
}
