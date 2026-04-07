// ---- API response wrapper ----

export interface PaginatedResponse<T> {
  page: number
  per_page: number
  total: number
  total_pages: number
  results: T[]
}

// ---- Index & Torrent ----

export interface Torrent {
  id: string
  index_id: string
  title: string
  magnet_link: string
  size_mb: number
  seeders: number
  leechers: number
  resolution: string | null
  codec: string | null
  release_type: string | null
  tracker_url: string | null
  score: number
  blacklisted: boolean
  blacklisted_reason: string | null
  created_at: string
}

export interface Index {
  id: string
  imdb_id: string
  season: number | null
  selected_torrent: string | null
  torrents: Torrent[]
  user_id: string
  created_at: string
}

// ---- Job ----

export type JobStage =
  | "pending"
  | "indexing"
  | "downloading"
  | "sterilizing"
  | "saving"
  | "cleanup"
  | "finished"
  | "failed"

export interface Job {
  id: string
  imdb_id: string
  title: string
  poster_path: string | null
  season: number | null
  current_stage: JobStage
  last_log: string
  preferences: Record<string, unknown> | null
  progress: Record<string, unknown>
  user_id: string
  created_at: string
  updated_at: string
}

// ---- TMDB ----

export interface PosterItem {
  id: number
  media_type: "movie" | "tv"
  title: string
  poster_path: string
  vote_average: number
  year?: string
}

export interface TMDBMeta {
  tmdbId: number
  title: string
  year: string
  posterPath: string | null
  overview: string
  mediaType: "movie" | "tv"
}

export interface TMDBSearchResult {
  id: number
  title?: string
  name?: string
  media_type: "movie" | "tv" | "person"
  release_date?: string
  first_air_date?: string
  vote_average: number
  poster_path: string | null
  overview: string
}

export interface TMDBSearchResponse {
  results: TMDBSearchResult[]
  page: number
  total_pages: number
  total_results: number
}

export interface TrendingResponse {
  page: number
  total_pages: number
  total_results: number
  results: PosterItem[]
}

export interface DiscoverRow {
  id: string
  title: string
  items: PosterItem[]
}

export interface DiscoverFeed {
  rows: DiscoverRow[]
}

// ---- Grouped indexes by imdb_id ----

export interface GroupedIndex {
  imdb_id: string
  indexes: Index[]
  meta?: TMDBMeta
}
