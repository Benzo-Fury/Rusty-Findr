use serde::Serialize;

use crate::classes::models::torrent::Torrent;
use crate::classes::models::ts;

#[derive(Serialize)]
pub struct Index {
    pub id: uuid::Uuid,
    pub imdb_id: String,
    pub season: Option<i32>,
    /// FK to the chosen torrent. None until the ranking stage selects one.
    pub selected_torrent: Option<uuid::Uuid>,
    pub torrents: Vec<Torrent>,
    /// FK to the user, better-auth stores user ID's as strings not UUID's.
    pub user_id: String,
    pub created_at: String,
}

impl Index {
    pub fn new(
        imdb_id: &str,
        season: Option<i32>,
        user_id: &str,
        torrents: Vec<Torrent>,
        selected_torrent: Option<uuid::Uuid>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            imdb_id: imdb_id.to_string(),
            season,
            selected_torrent,
            torrents,
            user_id: user_id.to_string(),
            created_at: String::new(),
        }
    }

    /// Returns the selected torrent, or None if no torrent is selected or the ID is missing.
    pub fn get_selected_torrent(&self) -> Option<&Torrent> {
        let id = self.selected_torrent?;
        self.torrents.iter().find(|t| t.id == id)
    }

    /// Select the next highest-scored, non-blacklisted torrent (excluding the
    /// current selection). Updates both in-memory state and the database.
    /// Returns a reference to the new torrent, or None if no candidates remain.
    pub async fn select_next_torrent(&mut self) -> Result<Option<&Torrent>, sqlx::Error> {
        let current_id = self.selected_torrent;

        // Torrents are stored ORDER BY score DESC, so the first match is the best
        let next_id = self
            .torrents
            .iter()
            .find(|t| !t.blacklisted && Some(t.id) != current_id)
            .map(|t| t.id);

        match next_id {
            Some(id) => {
                self.selected_torrent = Some(id);

                let pool = Self::pool();
                sqlx::query("UPDATE indexes SET selected_torrent = $1 WHERE id = $2")
                    .bind(id)
                    .bind(self.id)
                    .execute(pool)
                    .await?;

                Ok(self.torrents.iter().find(|t| t.id == id))
            }
            None => {
                self.selected_torrent = None;
                Ok(None)
            }
        }
    }

    /// Load index and its torrents from database using ID.
    pub async fn from_id(id: &str) -> Result<Index, sqlx::Error> {
        let pool = Self::pool();

        let uuid = uuid::Uuid::parse_str(id).map_err(|e| {
            tracing::error!("Invalid index ID '{id}': {e}");
            sqlx::Error::Decode(Box::new(e))
        })?;

        let row = sqlx::query_as::<_, IndexRow>(concat!(
            "SELECT id, imdb_id, season, selected_torrent, user_id, ",
            ts!("created_at"), " as created_at ",
            "FROM indexes WHERE id = $1",
        ))
        .bind(uuid)
        .fetch_one(pool)
        .await?;

        let torrents = sqlx::query_as::<_, Torrent>(concat!(
            "SELECT id, index_id, title, magnet_link, size_mb, seeders, leechers, resolution, codec, release_type, tracker_url, score, blacklisted, blacklisted_reason, ",
            ts!("created_at"), " as created_at ",
            "FROM torrents WHERE index_id = $1 ORDER BY score DESC",
        ))
        .bind(row.id)
        .fetch_all(pool)
        .await?;

        Ok(row.into_index(torrents))
    }

    /// Load index and its torrents from database by IMDb ID and season.
    pub async fn from_imdb(imdb_id: &str, season: Option<i32>) -> Result<Index, sqlx::Error> {
        let pool = Self::pool();

        let row = match season {
            Some(s) => {
                sqlx::query_as::<_, IndexRow>(concat!(
                    "SELECT id, imdb_id, season, selected_torrent, user_id, ",
                    ts!("created_at"), " as created_at ",
                    "FROM indexes WHERE imdb_id = $1 AND season = $2",
                ))
                .bind(imdb_id)
                .bind(s)
                .fetch_one(pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, IndexRow>(concat!(
                    "SELECT id, imdb_id, season, selected_torrent, user_id, ",
                    ts!("created_at"), " as created_at ",
                    "FROM indexes WHERE imdb_id = $1 AND season IS NULL",
                ))
                .bind(imdb_id)
                .fetch_one(pool)
                .await?
            }
        };

        let torrents = sqlx::query_as::<_, Torrent>(concat!(
            "SELECT id, index_id, title, magnet_link, size_mb, seeders, leechers, resolution, codec, release_type, tracker_url, score, blacklisted, blacklisted_reason, ",
            ts!("created_at"), " as created_at ",
            "FROM torrents WHERE index_id = $1 ORDER BY score DESC",
        ))
        .bind(row.id)
        .fetch_all(pool)
        .await?;

        Ok(row.into_index(torrents))
    }

    /// Find all indexes for a given IMDb ID (all seasons).
    pub async fn find_all_by_imdb(imdb_id: &str) -> Result<Vec<Index>, sqlx::Error> {
        let pool = Self::pool();

        let rows = sqlx::query_as::<_, IndexRow>(concat!(
            "SELECT id, imdb_id, season, selected_torrent, user_id, ",
            ts!("created_at"), " as created_at ",
            "FROM indexes WHERE imdb_id = $1 ORDER BY season ASC NULLS FIRST",
        ))
        .bind(imdb_id)
        .fetch_all(pool)
        .await?;

        let mut indexes = Vec::with_capacity(rows.len());
        for row in rows {
            let torrents = sqlx::query_as::<_, Torrent>(concat!(
                "SELECT id, index_id, title, magnet_link, size_mb, seeders, leechers, \
                 resolution, codec, release_type, tracker_url, score, blacklisted, \
                 blacklisted_reason, ",
                ts!("created_at"), " as created_at ",
                "FROM torrents WHERE index_id = $1 ORDER BY score DESC",
            ))
            .bind(row.id)
            .fetch_all(pool)
            .await?;

            indexes.push(row.into_index(torrents));
        }

        Ok(indexes)
    }

    /// Upsert this index and replace all associated torrents in a single transaction.
    pub async fn save(&self) -> Result<(), sqlx::Error> {
        let pool = Self::pool();
        let mut tx = pool.begin().await?;

        // Upsert the index row without selected_torrent (torrents don't exist yet)
        sqlx::query(
            "INSERT INTO indexes (id, imdb_id, season, selected_torrent, user_id)
             VALUES ($1, $2, $3, NULL, $4)
             ON CONFLICT (imdb_id, season)
             DO UPDATE SET selected_torrent = NULL",
        )
        .bind(self.id)
        .bind(&self.imdb_id)
        .bind(self.season)
        .bind(&self.user_id)
        .execute(&mut *tx)
        .await?;

        // Remove old torrents and insert new ones
        sqlx::query("DELETE FROM torrents WHERE index_id = $1")
            .bind(self.id)
            .execute(&mut *tx)
            .await?;

        for torrent in &self.torrents {
            sqlx::query(
                "INSERT INTO torrents (id, index_id, title, magnet_link, size_mb, seeders, leechers, resolution, codec, release_type, tracker_url, score, blacklisted, blacklisted_reason)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
            )
            .bind(torrent.id)
            .bind(self.id)
            .bind(&torrent.title)
            .bind(&torrent.magnet_link)
            .bind(torrent.size_mb)
            .bind(torrent.seeders)
            .bind(torrent.leechers)
            .bind(&torrent.resolution)
            .bind(&torrent.codec)
            .bind(&torrent.release_type)
            .bind(&torrent.tracker_url)
            .bind(torrent.score)
            .bind(torrent.blacklisted)
            .bind(&torrent.blacklisted_reason)
            .execute(&mut *tx)
            .await?;
        }

        // Now that torrents exist, set the selected torrent
        if let Some(selected_id) = self.selected_torrent {
            sqlx::query("UPDATE indexes SET selected_torrent = $1 WHERE id = $2")
                .bind(selected_id)
                .bind(self.id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        tracing::debug!("Saved index {} with {} torrents", self.id, self.torrents.len());

        Ok(())
    }

    /// List all indexes with their torrents, paginated. Returns (indexes, total_count).
    pub async fn list(page: u32, per_page: u32) -> Result<(Vec<Index>, i64), sqlx::Error> {
        let pool = Self::pool();
        let offset = (page.saturating_sub(1)) as i64 * per_page as i64;
        let limit = per_page as i64;

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM indexes")
            .fetch_one(pool)
            .await?;

        let rows = sqlx::query_as::<_, IndexRow>(concat!(
            "SELECT id, imdb_id, season, selected_torrent, user_id, ",
            ts!("created_at"), " as created_at ",
            "FROM indexes ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let mut indexes = Vec::with_capacity(rows.len());
        for row in rows {
            let torrents = sqlx::query_as::<_, Torrent>(concat!(
                "SELECT id, index_id, title, magnet_link, size_mb, seeders, leechers, \
                 resolution, codec, release_type, tracker_url, score, blacklisted, \
                 blacklisted_reason, ",
                ts!("created_at"), " as created_at ",
                "FROM torrents WHERE index_id = $1 ORDER BY score DESC",
            ))
            .bind(row.id)
            .fetch_all(pool)
            .await?;

            indexes.push(row.into_index(torrents));
        }

        Ok((indexes, total.0))
    }

    /// Delete an index by ID. Returns true if a row was deleted.
    pub async fn delete(id: uuid::Uuid) -> Result<bool, sqlx::Error> {
        let pool = Self::pool();
        let result = sqlx::query("DELETE FROM indexes WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    fn pool() -> &'static sqlx::Pool<sqlx::Postgres> {
        &crate::classes::database::Database::get().pool
    }
}

/// Intermediate row type for sqlx::FromRow mapping (no torrents relation).
#[derive(sqlx::FromRow)]
struct IndexRow {
    id: uuid::Uuid,
    imdb_id: String,
    season: Option<i32>,
    selected_torrent: Option<uuid::Uuid>,
    user_id: String,
    created_at: String,
}

impl IndexRow {
    fn into_index(self, torrents: Vec<Torrent>) -> Index {
        Index {
            id: self.id,
            imdb_id: self.imdb_id,
            season: self.season,
            selected_torrent: self.selected_torrent,
            torrents,
            user_id: self.user_id,
            created_at: self.created_at,
        }
    }
}
