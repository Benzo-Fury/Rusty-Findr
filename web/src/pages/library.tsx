import * as React from "react"
import { Search, ChevronLeft, ChevronRight } from "lucide-react"
import { MediaCard } from "@/components/media-card"
import { Skeleton } from "@/components/ui/skeleton"
import { Button } from "@/components/ui/button"
import { TitleDialog } from "@/components/title-dialog"
import { fetchIndexes } from "@/lib/api"
import { useTMDBMeta } from "@/lib/hooks"
import type { Index, PosterItem } from "@/lib/types"

export function LibraryPage() {
  const [indexes, setIndexes] = React.useState<Index[]>([])
  const [loading, setLoading] = React.useState(true)
  const [page, setPage] = React.useState(1)
  const [totalPages, setTotalPages] = React.useState(1)
  const { meta, loadingIds, fetchMeta } = useTMDBMeta()
  const [titleItem, setTitleItem] = React.useState<PosterItem | null>(null)

  React.useEffect(() => {
    setLoading(true)
    fetchIndexes(page, 40).then((data) => {
      setIndexes(data.results)
      setTotalPages(data.total_pages)
      setLoading(false)
    }).catch(() => setLoading(false))
  }, [page])

  React.useEffect(() => {
    const uniqueImdbIds = [...new Set(indexes.map((i) => i.imdb_id))]
    uniqueImdbIds.forEach(fetchMeta)
  }, [indexes, fetchMeta])

  const grouped = React.useMemo(() => {
    const map = new Map<string, Index[]>()
    for (const idx of indexes) {
      const existing = map.get(idx.imdb_id) || []
      existing.push(idx)
      map.set(idx.imdb_id, existing)
    }
    return Array.from(map.entries()).map(([imdb_id, idxs]) => ({
      imdb_id,
      indexes: idxs,
      meta: meta[imdb_id],
    }))
  }, [indexes, meta])

  return (
    <div className="mx-auto max-w-[1600px] px-4 py-6 lg:px-6 lg:py-8">
      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-2">
          {!loading && (
            <span className="text-sm text-muted-foreground">
              {indexes.length} items
            </span>
          )}
        </div>
      </div>

      {loading ? (
        <div className="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {Array.from({ length: 20 }).map((_, i) => (
            <Skeleton key={i} className="aspect-[2/3] rounded-xl" />
          ))}
        </div>
      ) : grouped.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <Search className="mb-4 size-12 text-muted-foreground/50" />
          <h2 className="mb-1 text-lg font-medium">Your library is empty</h2>
          <p className="text-sm text-muted-foreground">
            Search for movies or TV shows to start building your library.
          </p>
        </div>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {grouped.map((group) =>
              !group.meta && loadingIds.has(group.imdb_id) ? (
                <Skeleton key={group.imdb_id} className="aspect-[2/3] rounded-xl" />
              ) : (
                <MediaCard
                  key={group.imdb_id}
                  title={group.meta?.title || group.imdb_id}
                  year={group.meta?.year}
                  posterPath={group.meta?.posterPath || null}
                  mediaType={group.meta?.mediaType || "movie"}
                  onClick={() => {
                    if (group.meta) {
                      setTitleItem({
                        id: group.meta.tmdbId,
                        media_type: group.meta.mediaType,
                        title: group.meta.title,
                        poster_path: group.meta.posterPath || "",
                        vote_average: 0,
                        year: group.meta.year,
                      })
                    }
                  }}
                />
              )
            )}
          </div>

          {totalPages > 1 && (
            <div className="mt-6 flex items-center justify-center gap-2">
              <Button
                variant="outline"
                size="icon-sm"
                disabled={page <= 1}
                onClick={() => setPage((p) => p - 1)}
              >
                <ChevronLeft className="size-4" />
              </Button>
              <span className="text-sm text-muted-foreground">
                Page {page} of {totalPages}
              </span>
              <Button
                variant="outline"
                size="icon-sm"
                disabled={page >= totalPages}
                onClick={() => setPage((p) => p + 1)}
              >
                <ChevronRight className="size-4" />
              </Button>
            </div>
          )}
        </>
      )}

      {titleItem && (
        <TitleDialog
          item={titleItem}
          onClose={() => setTitleItem(null)}
          onItemClick={(item) => setTitleItem(item)}
        />
      )}
    </div>
  )
}

