import * as React from "react"
import type { PosterItem, TMDBMeta } from "./types"
import { fetchTrending, fetchTopRated } from "./api"

export type DiscoverTab = "trending" | "top-rated"

export function useTMDBDiscover(tab: DiscoverTab, mediaType: string, initialItems?: PosterItem[]) {
  const [items, setItems] = React.useState<PosterItem[]>([])
  const [loading, setLoading] = React.useState(true)
  const [loadingMore, setLoadingMore] = React.useState(false)
  const [hasMore, setHasMore] = React.useState(true)
  const pageRef = React.useRef(1)
  const loadingMoreRef = React.useRef(false)
  const hasMoreRef = React.useRef(true)

  React.useEffect(() => {
    setHasMore(true)
    setLoadingMore(false)
    hasMoreRef.current = true
    loadingMoreRef.current = false

    if (initialItems) {
      setItems(initialItems)
      setLoading(false)
      pageRef.current = 1
      return
    }

    setItems([])
    setLoading(true)
    pageRef.current = 1

    const fetcher = tab === "trending" ? fetchTrending : (p: number) => fetchTopRated(p, mediaType)
    fetcher(1).then((data) => {
      setItems(data.results)
      const more = data.page < data.total_pages
      setHasMore(more)
      hasMoreRef.current = more
      setLoading(false)
    }).catch(() => setLoading(false))
  }, [tab, mediaType, initialItems])

  const loadMore = React.useCallback(() => {
    if (loadingMoreRef.current || !hasMoreRef.current) return
    loadingMoreRef.current = true
    setLoadingMore(true)
    const nextPage = pageRef.current + 1
    const fetcher = tab === "trending" ? fetchTrending : (p: number) => fetchTopRated(p, mediaType)
    fetcher(nextPage).then((data) => {
      pageRef.current = nextPage
      setItems((prev) => {
        const seen = new Set(prev.map((i) => `${i.media_type}-${i.id}`))
        const unique = data.results.filter((i) => !seen.has(`${i.media_type}-${i.id}`))
        return [...prev, ...unique]
      })
      const more = data.page < data.total_pages
      setHasMore(more)
      hasMoreRef.current = more
      loadingMoreRef.current = false
      setLoadingMore(false)
    }).catch(() => {
      loadingMoreRef.current = false
      setLoadingMore(false)
    })
  }, [tab, mediaType])

  return { items, loading, loadingMore, hasMore, loadMore }
}

export function useInfiniteScroll(
  loadMore: () => void,
  hasMore: boolean,
  isLoading: boolean,
) {
  const nodeRef = React.useRef<HTMLDivElement | null>(null)
  const observerRef = React.useRef<IntersectionObserver | null>(null)
  const callbackRef = React.useRef(loadMore)
  callbackRef.current = loadMore
  const hasMoreRef = React.useRef(hasMore)
  hasMoreRef.current = hasMore
  const isLoadingRef = React.useRef(isLoading)
  isLoadingRef.current = isLoading

  const attachObserver = React.useCallback((node: HTMLDivElement | null) => {
    if (observerRef.current) {
      observerRef.current.disconnect()
      observerRef.current = null
    }
    nodeRef.current = node
    if (!node) return

    observerRef.current = new IntersectionObserver(
      (entries) => {
        if (
          entries[0]?.isIntersecting &&
          hasMoreRef.current &&
          !isLoadingRef.current
        ) {
          callbackRef.current()
        }
      },
      { rootMargin: "800px" },
    )
    observerRef.current.observe(node)
  }, [])

  // Re-attach after loading finishes so the observer re-evaluates
  // whether the sentinel is still intersecting (fires on observe).
  React.useEffect(() => {
    if (!isLoading && hasMore && nodeRef.current) {
      attachObserver(nodeRef.current)
    }
  }, [isLoading, hasMore, attachObserver])

  return attachObserver
}

export function useTMDBMeta() {
  const [meta, setMeta] = React.useState<Record<string, TMDBMeta>>({})
  const [loadingIds, setLoadingIds] = React.useState<Set<string>>(new Set())
  const pending = React.useRef(new Set<string>())

  const fetchMeta = React.useCallback((imdbId: string) => {
    if (meta[imdbId] || pending.current.has(imdbId)) return
    pending.current.add(imdbId)
    setLoadingIds((prev) => new Set(prev).add(imdbId))

    fetch(`/api/tmdb/find/${imdbId}`)
      .then((r) => r.json())
      .then((data) => {
        const movie = data.movie_results?.[0]
        const tv = data.tv_results?.[0]
        const item = movie || tv
        if (!item) return

        setMeta((prev) => ({
          ...prev,
          [imdbId]: {
            tmdbId: item.id,
            title: item.title || item.name,
            year: (item.release_date || item.first_air_date || "").slice(0, 4),
            posterPath: item.poster_path,
            overview: item.overview,
            mediaType: movie ? "movie" : "tv",
          },
        }))
      })
      .catch(() => {})
      .finally(() => {
        setLoadingIds((prev) => {
          const next = new Set(prev)
          next.delete(imdbId)
          return next
        })
      })
  }, [meta])

  return { meta, loadingIds, fetchMeta }
}
