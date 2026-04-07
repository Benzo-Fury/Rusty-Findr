import * as React from "react"
import { MediaRow } from "@/components/media-row"
import { MediaCard } from "@/components/media-card"
import { Skeleton } from "@/components/ui/skeleton"
import { TitleDialog } from "@/components/title-dialog"
import { fetchDiscoverFeed } from "@/lib/api"
import { useTMDBDiscover, useInfiniteScroll } from "@/lib/hooks"
import type { PosterItem, DiscoverFeed } from "@/lib/types"
import { useNavigate, useParams } from "react-router-dom"

export function DiscoverPage() {
  const [feed, setFeed] = React.useState<DiscoverFeed | null>(null)
  const [loading, setLoading] = React.useState(true)
  const navigate = useNavigate()
  const params = useParams<{ mediaType?: string; id?: string }>()

  function openItem(item: PosterItem) {
    navigate(`/discover/${item.media_type}/${item.id}`)
  }

  function closeItem() {
    navigate("/discover")
  }

  React.useEffect(() => {
    fetchDiscoverFeed()
      .then(setFeed)
      .catch(() => {})
      .finally(() => setLoading(false))
  }, [])

  // Extract trending items from the feed (used for both the grid and item lookup)
  const feedTrendingItems = React.useMemo(
    () => feed?.rows.find((r) => r.id === "trending")?.items ?? [],
    [feed],
  )

  // Build selectedItem from URL params
  const selectedItem = React.useMemo<PosterItem | null>(() => {
    if (!params.mediaType || !params.id) return null
    const id = Number(params.id)
    if (isNaN(id)) return null
    const mediaType = params.mediaType as "movie" | "tv"
    if (mediaType !== "movie" && mediaType !== "tv") return null

    // Try to find the item in the feed for richer data
    if (feed) {
      for (const row of feed.rows) {
        const found = row.items.find(
          (i) => i.id === id && i.media_type === mediaType,
        )
        if (found) return found
      }
    }

    // Fallback: construct a minimal PosterItem from URL params
    return { id, media_type: mediaType, title: "", poster_path: "", vote_average: 0 }
  }, [params.mediaType, params.id, feed])

  if (loading) return <DiscoverSkeleton />

  return (
    <div className="space-y-8 py-6">
      {feed?.rows.map((row) => (
        <MediaRow
          key={row.id}
          title={row.title}
          items={row.items}
          ranked={row.id === "top-10-movies"}
          onItemClick={openItem}
        />
      ))}

      {/* More Trending - infinite scroll grid, seeded from the feed's trending row */}
      {feed && (
        <TrendingGrid
          initialItems={feedTrendingItems}
          onItemClick={openItem}
        />
      )}

      {selectedItem && (
        <TitleDialog
          item={selectedItem}
          onClose={closeItem}
          onItemClick={openItem}
        />
      )}
    </div>
  )
}

function TrendingGrid({ initialItems, onItemClick }: { initialItems: PosterItem[]; onItemClick: (item: PosterItem) => void }) {
  const { items: trendingItems, loadingMore, hasMore, loadMore } = useTMDBDiscover("trending", "all", initialItems)
  const sentinelRef = useInfiniteScroll(loadMore, hasMore, loadingMore)

  if (trendingItems.length === 0) return null

  return (
    <div className="px-4 lg:px-6">
      <h2 className="mb-4 text-xl font-semibold">More Trending</h2>
      <div className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6">
        {trendingItems.map((item) => (
          <MediaCard
            key={`${item.media_type}-${item.id}`}
            title={item.title}
            year={item.year}
            posterPath={item.poster_path}
            mediaType={item.media_type}
            rating={item.vote_average}
            onClick={() => onItemClick(item)}
            compact
          />
        ))}
      </div>
      {loadingMore && (
        <div className="mt-3 grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="aspect-[2/3] w-full rounded-xl" />
          ))}
        </div>
      )}
      <div ref={sentinelRef} className="h-8" />
    </div>
  )
}

function DiscoverSkeleton() {
  return (
    <div className="space-y-8 py-6">
      {Array.from({ length: 5 }).map((_, i) => (
        <div key={i} className="px-4 lg:px-6">
          <Skeleton className="mb-3 h-6 w-48" />
          <div className="flex gap-3">
            {Array.from({ length: 7 }).map((_, j) => (
              <Skeleton
                key={j}
                className="aspect-[2/3] w-36 flex-shrink-0 rounded-xl sm:w-40 md:w-44 lg:w-48"
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
