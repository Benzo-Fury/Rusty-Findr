import type {
  PaginatedResponse,
  Index,
  Job,
  TrendingResponse,
  TMDBSearchResponse,
  DiscoverFeed,
} from "./types"

export type { Index }

async function fetchJSON<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, init)
  if (!res.ok) {
    const body = await res.json().catch(() => ({}))
    throw new Error(body.error || `Request failed: ${res.status}`)
  }
  return res.json()
}

// ---- Indexes ----

export function fetchIndexes(page = 1, perPage = 40) {
  return fetchJSON<PaginatedResponse<Index>>(
    `/api/indexes?page=${page}&per_page=${perPage}`
  )
}

// ---- Jobs ----

export function fetchJobs(page = 1, perPage = 50) {
  return fetchJSON<PaginatedResponse<Job>>(
    `/api/jobs?page=${page}&per_page=${perPage}`
  )
}

export async function deleteJob(id: string) {
  const res = await fetch(`/api/jobs/${id}`, { method: "DELETE" })
  if (!res.ok && res.status !== 204) {
    const body = await res.json().catch(() => ({}))
    throw new Error(body.error || "Failed to delete job")
  }
}

export function createJob(imdbId: string, title: string, posterPath: string | null, season?: number) {
  return fetchJSON<{ id: string; imdb_id: string; season: number | null; current_stage: string }>(
    "/api/jobs",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ imdb_id: imdbId, title, poster_path: posterPath, season }),
    }
  )
}

// ---- TMDB ----

export function fetchTrending(page = 1) {
  return fetchJSON<TrendingResponse>(`/api/tmdb/trending?page=${page}`)
}

export function fetchTopRated(page = 1, type?: string) {
  const params = new URLSearchParams({ page: String(page) })
  if (type && type !== "all") params.set("type", type)
  return fetchJSON<TrendingResponse>(`/api/tmdb/top-rated?${params}`)
}

export function searchTMDB(query: string) {
  return fetchJSON<TMDBSearchResponse>(
    `/api/tmdb/search?q=${encodeURIComponent(query)}`
  )
}

export function fetchTMDBDetails(mediaType: "movie" | "tv", id: number, init?: RequestInit) {
  return fetchJSON<Record<string, unknown>>(
    `/api/tmdb/details/${mediaType}/${id}`,
    init,
  )
}

export function fetchDiscoverFeed() {
  return fetchJSON<DiscoverFeed>("/api/tmdb/discover-feed")
}

// ---- Index lookup ----

export function lookupIndexes(imdbId: string, init?: RequestInit) {
  return fetchJSON<{ indexes: Index[] }>(`/api/indexes/lookup/${imdbId}`, init)
}
