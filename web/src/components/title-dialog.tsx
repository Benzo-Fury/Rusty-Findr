import * as React from "react"
import { X, Download, TriangleAlert } from "lucide-react"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { MediaCard } from "@/components/media-card"
import { Skeleton } from "@/components/ui/skeleton"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { TorrentText } from "@/components/torrent-text"
import { lookupIndexes, fetchTMDBDetails, createJob } from "@/lib/api"
import type { PosterItem, Index } from "@/lib/types"
import { useNavigate } from "react-router-dom"

const BACKDROP_BASE = "https://image.tmdb.org/t/p/w1280"
const POSTER_BASE = "https://image.tmdb.org/t/p/w342"

interface Video {
  key: string
  site: string
  type: string
  official: boolean
}

interface CastMember {
  name: string
  character: string
  profile_path: string | null
}

interface Recommendation {
  id: number
  title?: string
  name?: string
  media_type: string
  poster_path: string | null
  vote_average: number
  release_date?: string
  first_air_date?: string
}

interface TitleDialogProps {
  item: PosterItem
  onClose: () => void
  onItemClick: (item: PosterItem) => void
}

export function TitleDialog({ item, onClose, onItemClick }: TitleDialogProps) {
  const navigate = useNavigate()
  const [details, setDetails] = React.useState<Record<string, unknown> | null>(null)
  const [indexes, setIndexes] = React.useState<Index[]>([])
  const [status, setStatus] = React.useState<"loading" | "indexed" | "not-indexed">("loading")
  const [creating, setCreating] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [season, setSeason] = React.useState(1)
  const contentRef = React.useRef<HTMLDivElement>(null)

  React.useEffect(() => {
    setStatus("loading")
    setDetails(null)
    setIndexes([])
    setError(null)
    setSeason(1)
    contentRef.current?.scrollTo(0, 0)

    const controller = new AbortController()
    const opts = { signal: controller.signal }

    async function load() {
      try {
        const data = await fetchTMDBDetails(
          item.media_type as "movie" | "tv",
          item.id,
          opts,
        )

        setDetails(data)

        const imdbId = (data.imdb_id || (data.external_ids as Record<string, unknown>)?.imdb_id) as string | undefined

        if (!imdbId) {
          setStatus("not-indexed")
          return
        }

        const result = await lookupIndexes(imdbId, opts)

        if (result.indexes.length > 0) {
          setIndexes(result.indexes)
          setStatus("indexed")
        } else {
          setStatus("not-indexed")
        }
      } catch (e) {
        if (!controller.signal.aborted) setStatus("not-indexed")
      }
    }

    load()
    return () => controller.abort()
  }, [item.media_type, item.id])

  async function handleIndex() {
    if (!details) return
    setCreating(true)
    setError(null)

    try {
      const imdbId = (details.imdb_id || (details.external_ids as Record<string, unknown>)?.imdb_id) as string | undefined
      if (!imdbId) {
        setError("Could not find IMDb ID")
        setCreating(false)
        return
      }

      const jobTitle = (details.title || details.name || item.title) as string
      const posterPath = (details.poster_path || item.poster_path || null) as string | null
      const s = item.media_type === "tv" ? season : undefined
      await createJob(imdbId, jobTitle, posterPath, s)
      onClose()
      navigate("/jobs")
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create job")
    }
    setCreating(false)
  }

  const title = (details?.title || details?.name || item.title) as string
  const overview = (details?.overview || "") as string
  const releaseDate = (details?.release_date || details?.first_air_date || "") as string
  const runtime = details?.runtime as number | undefined
  const episodeRuntime = (details?.episode_run_time as number[] | undefined)?.[0]
  const displayRuntime = runtime || episodeRuntime
  const genres = (details?.genres || []) as { id: number; name: string }[]
  const voteAverage = (details?.vote_average || item.vote_average) as number
  const numberOfSeasons = (details?.number_of_seasons || 1) as number
  const backdropPath = details?.backdrop_path as string | undefined
  const tagline = details?.tagline as string | undefined

  const videos = ((details?.videos as Record<string, unknown>)?.results || []) as Video[]
  const trailer = videos.find(
    (v) => v.site === "YouTube" && v.type === "Trailer" && v.official,
  ) || videos.find(
    (v) => v.site === "YouTube" && v.type === "Trailer",
  ) || videos.find(
    (v) => v.site === "YouTube",
  )

  const cast = ((details?.credits as Record<string, unknown>)?.cast || []) as CastMember[]
  const topCast = cast.slice(0, 6)

  const rawRecs = ((details?.recommendations as Record<string, unknown>)?.results || []) as Recommendation[]
  const recommendations: PosterItem[] = rawRecs
    .filter((r) => r.poster_path)
    .slice(0, 20)
    .map((r) => ({
      id: r.id,
      media_type: (r.media_type === "tv" ? "tv" : "movie") as "movie" | "tv",
      title: r.title || r.name || "",
      poster_path: r.poster_path!,
      vote_average: r.vote_average,
      year: (r.release_date || r.first_air_date || "").slice(0, 4) || undefined,
    }))

  const availabilityWarning = React.useMemo(() => {
    if (!details) return null

    if (item.media_type === "movie") {
      const releaseDates = (details.release_dates as { results?: { iso_3166_1: string; release_dates: { type: number; release_date: string }[] }[] })?.results
      if (releaseDates) {
        const usRelease = releaseDates.find((r) => r.iso_3166_1 === "US")
        const releases = usRelease?.release_dates ?? releaseDates.flatMap((r) => r.release_dates)
        const now = new Date()

        const hasTheatrical = releases.some(
          (r) => (r.type === 2 || r.type === 3) && new Date(r.release_date) <= now,
        )
        const hasDigitalOrPhysical = releases.some(
          (r) => (r.type === 4 || r.type === 5) && new Date(r.release_date) <= now,
        )

        if (hasTheatrical && !hasDigitalOrPhysical) {
          return <>This movie is currently only in cinemas. <TorrentText>Torrents</TorrentText> may be unavailable or low quality.</>
        }
      }
    }

    const releaseDateStr = (details.release_date || details.first_air_date) as string | undefined
    if (releaseDateStr) {
      const releaseTime = new Date(releaseDateStr).getTime()
      const thirtyDaysMs = 30 * 24 * 60 * 60 * 1000
      if (Date.now() - releaseTime < thirtyDaysMs && Date.now() >= releaseTime) {
        return <>This title released very recently. <TorrentText>Torrents</TorrentText> may be unavailable or low quality.</>
      }
    }

    return null
  }, [details, item.media_type])

  return (
    <Dialog open onOpenChange={() => onClose()}>
      <DialogContent className="max-w-4xl gap-0 overflow-hidden p-0 max-h-[90vh]">
        <div ref={contentRef} className="overflow-y-auto max-h-[90vh]">
          {status === "loading" ? (
            <DialogSkeleton onClose={onClose} />
          ) : status === "indexed" ? (
            <IndexedContent
              item={item}
              details={details!}
              indexes={indexes}
              onClose={onClose}
            />
          ) : (
            <>
              <div className="relative aspect-video w-full bg-muted">
                {backdropPath ? (
                  <img
                    src={`${BACKDROP_BASE}${backdropPath}`}
                    alt={title}
                    className="size-full object-cover"
                  />
                ) : null}

                <div className="absolute inset-0 bg-gradient-to-t from-background via-background/50 to-transparent" />

                <button
                  onClick={onClose}
                  className="absolute top-3 right-3 flex size-9 items-center justify-center rounded-full bg-black/60 text-white transition-colors hover:bg-black/80"
                >
                  <X className="size-5" />
                </button>

                <div className="absolute inset-x-0 bottom-0 flex items-end gap-5 p-6">
                  {item.poster_path && (
                    <img
                      src={`${POSTER_BASE}${item.poster_path}`}
                      alt={title}
                      className="hidden w-28 flex-shrink-0 rounded-lg border border-white/10 shadow-xl sm:block md:w-32"
                    />
                  )}

                  <div className="min-w-0 flex-1">
                    <h1 className="text-2xl font-bold text-foreground sm:text-3xl lg:text-4xl">
                      {title}
                    </h1>

                    {tagline && (
                      <p className="mt-1 text-sm italic text-muted-foreground">{tagline}</p>
                    )}

                    <div className="mt-4 flex flex-wrap items-center gap-3">
                      <Button
                        onClick={handleIndex}
                        disabled={creating}
                        className="gap-2"
                      >
                        <Download className="size-4" />
                        {creating ? "Creating..." : "Index"}
                      </Button>

                      {item.media_type === "tv" && numberOfSeasons > 0 && (
                        <select
                          value={season}
                          onChange={(e) => setSeason(Number(e.target.value))}
                          className="rounded-lg border border-border bg-background px-3 py-2 text-sm"
                        >
                          {Array.from({ length: numberOfSeasons }, (_, i) => i + 1).map((s) => (
                            <option key={s} value={s}>Season {s}</option>
                          ))}
                        </select>
                      )}
                    </div>

                    {error && (
                      <p className="mt-2 text-sm text-red-400">{error}</p>
                    )}
                  </div>
                </div>
              </div>

              {availabilityWarning && (
                <div className="mx-6 mt-4 flex cursor-default items-start gap-3 rounded-lg border border-amber-500/20 bg-amber-500/10 px-4 py-3">
                  <TriangleAlert className="mt-0.5 size-4 flex-shrink-0 text-amber-600 dark:text-amber-400" />
                  <p className="text-sm text-amber-600 dark:text-amber-400">{availabilityWarning}</p>
                </div>
              )}

              <div className="p-6">
                <div className="grid grid-cols-1 gap-6 md:grid-cols-[1fr_auto]">
                  <div>
                    <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                      {releaseDate && <span className="font-medium text-foreground">{releaseDate.slice(0, 4)}</span>}
                      {item.media_type === "tv" && numberOfSeasons > 0 && (
                        <span>{numberOfSeasons} {numberOfSeasons === 1 ? "Season" : "Seasons"}</span>
                      )}
                      {displayRuntime && (
                        <span>{displayRuntime}m</span>
                      )}
                      {voteAverage > 0 && (
                        <span className="flex items-center gap-1">
                          <span className="text-amber-400">&#9733;</span>
                          {voteAverage.toFixed(1)}
                        </span>
                      )}
                    </div>

                    {genres.length > 0 && (
                      <div className="mt-3 flex flex-wrap gap-1.5">
                        {genres.map((g) => (
                          <Badge key={g.id} variant="secondary">{g.name}</Badge>
                        ))}
                      </div>
                    )}

                    {overview && (
                      <p className="mt-4 text-sm leading-relaxed text-muted-foreground">
                        {overview}
                      </p>
                    )}
                  </div>

                  {topCast.length > 0 && (
                    <div className="min-w-0 text-sm md:w-56">
                      <p className="text-muted-foreground">
                        <span className="text-muted-foreground/60">Cast: </span>
                        <span className="text-foreground">
                          {topCast.map((c) => c.name).join(", ")}
                        </span>
                      </p>
                      <p className="mt-2 text-muted-foreground">
                        <span className="text-muted-foreground/60">Genres: </span>
                        <span className="text-foreground">
                          {genres.map((g) => g.name).join(", ")}
                        </span>
                      </p>
                    </div>
                  )}
                </div>

                {trailer && (
                  <div className="mt-6">
                    <h3 className="mb-3 text-lg font-semibold">Trailer</h3>
                    <div className="aspect-video w-full overflow-hidden rounded-lg">
                      <iframe
                        src={`https://www.youtube.com/embed/${trailer.key}`}
                        title={`${title} trailer`}
                        className="size-full"
                        allow="accelerometer; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                        allowFullScreen
                      />
                    </div>
                  </div>
                )}

                {recommendations.length > 0 && (
                  <div className="mt-6">
                    <h3 className="mb-3 text-lg font-semibold">More Like This</h3>
                    <div className="flex gap-3 overflow-x-auto pb-2 scrollbar-hide">
                      {recommendations.map((rec) => (
                        <div key={`${rec.media_type}-${rec.id}`} className="w-32 flex-shrink-0 sm:w-36">
                          <MediaCard
                            title={rec.title}
                            year={rec.year}
                            posterPath={rec.poster_path}
                            mediaType={rec.media_type}
                            rating={rec.vote_average}
                            onClick={() => onItemClick(rec)}
                          />
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}

function DialogSkeleton({ onClose }: { onClose: () => void }) {
  return (
    <>
      <div className="relative aspect-video w-full">
        <Skeleton className="size-full rounded-none" />
        <button
          onClick={onClose}
          className="absolute top-3 right-3 flex size-9 items-center justify-center rounded-full bg-black/60 text-white transition-colors hover:bg-black/80"
        >
          <X className="size-5" />
        </button>
        <div className="absolute inset-x-0 bottom-0 flex items-end gap-5 p-6">
          <Skeleton className="hidden h-40 w-28 flex-shrink-0 rounded-lg sm:block md:w-32" />
          <div className="min-w-0 flex-1 space-y-3">
            <Skeleton className="h-8 w-2/3" />
            <Skeleton className="h-4 w-1/3" />
            <Skeleton className="mt-4 h-10 w-28 rounded-md" />
          </div>
        </div>
      </div>
      <div className="p-6">
        <div className="flex items-center gap-3">
          <Skeleton className="h-4 w-10" />
          <Skeleton className="h-4 w-16" />
          <Skeleton className="h-4 w-10" />
        </div>
        <div className="mt-3 flex gap-2">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-6 w-20 rounded-full" />
          ))}
        </div>
        <div className="mt-4 space-y-2">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-3/4" />
        </div>
        <div className="mt-6">
          <Skeleton className="mb-3 h-6 w-24" />
          <Skeleton className="aspect-video w-full rounded-lg" />
        </div>
        <div className="mt-6">
          <Skeleton className="mb-3 h-6 w-32" />
          <div className="flex gap-3">
            {Array.from({ length: 5 }).map((_, i) => (
              <Skeleton key={i} className="h-48 w-32 flex-shrink-0 rounded-lg sm:w-36" />
            ))}
          </div>
        </div>
      </div>
    </>
  )
}

function IndexedContent({
  item: _item,
  details: _details,
  indexes: _indexes,
  onClose,
}: {
  item: PosterItem
  details: Record<string, unknown>
  indexes: Index[]
  onClose: () => void
}) {
  return (
    <>
      <div className="relative aspect-video w-full">
        <Skeleton className="size-full rounded-none" />
        <button
          onClick={onClose}
          className="absolute top-3 right-3 flex size-9 items-center justify-center rounded-full bg-black/60 text-white transition-colors hover:bg-black/80"
        >
          <X className="size-5" />
        </button>
        <div className="absolute inset-x-0 bottom-0 flex items-end gap-5 p-6">
          <Skeleton className="hidden h-40 w-28 flex-shrink-0 rounded-lg sm:block md:w-32" />
          <div className="min-w-0 flex-1 space-y-3">
            <Skeleton className="h-8 w-2/3" />
            <Skeleton className="h-4 w-1/3" />
          </div>
        </div>
      </div>

      <div className="p-6 space-y-6">
        <div className="flex items-center gap-3">
          <Skeleton className="h-5 w-16" />
          <Skeleton className="h-5 w-20" />
          <Skeleton className="h-5 w-12" />
        </div>

        <div className="space-y-2">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-3/4" />
        </div>

        <div>
          <Skeleton className="mb-3 h-5 w-32" />
          <div className="space-y-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="rounded-lg border p-4 space-y-2">
                <Skeleton className="h-4 w-3/4" />
                <div className="flex gap-2">
                  <Skeleton className="h-5 w-16 rounded-full" />
                  <Skeleton className="h-5 w-14 rounded-full" />
                  <Skeleton className="h-5 w-20 rounded-full" />
                  <Skeleton className="h-5 w-16 rounded-full" />
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </>
  )
}
