import * as React from "react"
import { X, Download, TriangleAlert, RefreshCw } from "lucide-react"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { MediaCard } from "@/components/media-card"
import { Skeleton } from "@/components/ui/skeleton"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { TorrentText } from "@/components/torrent-text"
import { lookupIndexes, fetchTMDBDetails, createJob, deleteIndex } from "@/lib/api"
import type { PosterItem, Index, Torrent } from "@/lib/types"
import { cn } from "@/lib/utils"
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
  item,
  details,
  indexes,
  onClose,
}: {
  item: PosterItem
  details: Record<string, unknown>
  indexes: Index[]
  onClose: () => void
}) {
  const navigate = useNavigate()
  const [activeSeason, setActiveSeason] = React.useState<number | null>(indexes[0]?.season ?? null)
  const [showNotImplemented, setShowNotImplemented] = React.useState(false)
  const [reindexing, setReindexing] = React.useState(false)
  const [reindexError, setReindexError] = React.useState<string | null>(null)

  const currentIndex = indexes.find((ix) => ix.season === activeSeason) ?? indexes[0]

  const title = (details.title || details.name || item.title) as string
  const backdropPath = details.backdrop_path as string | undefined
  const tagline = details.tagline as string | undefined
  const releaseDate = (details.release_date || details.first_air_date || "") as string
  const genres = (details.genres || []) as { id: number; name: string }[]
  const voteAverage = (details.vote_average || item.vote_average) as number

  async function handleReindex() {
    if (!currentIndex) return
    setReindexing(true)
    setReindexError(null)
    try {
      await deleteIndex(currentIndex.id)
      const posterPath = (details.poster_path || item.poster_path || null) as string | null
      const season = item.media_type === "tv" ? currentIndex.season ?? undefined : undefined
      await createJob(currentIndex.imdb_id, title, posterPath, season)
      onClose()
      navigate("/jobs")
    } catch (e) {
      setReindexError(e instanceof Error ? e.message : "Failed to re-index")
      setReindexing(false)
    }
  }

  const torrents = currentIndex?.torrents ?? []
  const selectedId = currentIndex?.selected_torrent ?? null

  return (
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
                onClick={handleReindex}
                disabled={reindexing}
                variant="outline"
                className="gap-2"
              >
                <RefreshCw className={cn("size-4", reindexing && "animate-spin")} />
                {reindexing ? "Re-indexing..." : "Re-Index"}
              </Button>

              {indexes.length > 1 && (
                <select
                  value={activeSeason ?? ""}
                  onChange={(e) => {
                    const val = e.target.value
                    setActiveSeason(val === "" ? null : Number(val))
                  }}
                  className="rounded-lg border border-border bg-background px-3 py-2 text-sm"
                >
                  {indexes.map((ix) => (
                    <option key={ix.id} value={ix.season ?? ""}>
                      {ix.season === null ? "Movie" : `Season ${ix.season}`}
                    </option>
                  ))}
                </select>
              )}
            </div>

            {reindexError && (
              <p className="mt-2 text-sm text-red-400">{reindexError}</p>
            )}
          </div>
        </div>
      </div>

      <div className="p-6 space-y-5">
        <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
          {releaseDate && (
            <span className="font-medium text-foreground">{releaseDate.slice(0, 4)}</span>
          )}
          <Badge variant="secondary" className="border-green-200 bg-green-50 text-green-700">
            Indexed
          </Badge>
          {voteAverage > 0 && (
            <span className="flex items-center gap-1">
              <span className="text-amber-400">&#9733;</span>
              {voteAverage.toFixed(1)}
            </span>
          )}
          {genres.slice(0, 3).map((g) => (
            <Badge key={g.id} variant="secondary">{g.name}</Badge>
          ))}
        </div>

        <div>
          <h3 className="mb-3 text-base font-semibold">
            Torrents{" "}
            {torrents.length > 0 && (
              <span className="font-normal text-sm text-muted-foreground">({torrents.length})</span>
            )}
          </h3>
          {torrents.length === 0 ? (
            <p className="text-sm text-muted-foreground">No torrents stored for this index.</p>
          ) : (
            <div className="space-y-2">
              {torrents.map((torrent) => (
                <TorrentRow
                  key={torrent.id}
                  torrent={torrent}
                  isSelected={torrent.id === selectedId}
                  onClick={() => setShowNotImplemented(true)}
                />
              ))}
            </div>
          )}
        </div>
      </div>

      {showNotImplemented && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
          onClick={() => setShowNotImplemented(false)}
        >
          <div className="absolute inset-0 bg-black/40" />
          <div
            className="relative w-full max-w-sm rounded-xl border bg-background p-6 shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="text-base font-semibold">Not Yet Implemented</h2>
            <p className="mt-2 text-sm text-muted-foreground">
              Torrent selection is planned and not yet implemented.
            </p>
            <Button className="mt-4 w-full" onClick={() => setShowNotImplemented(false)}>
              OK
            </Button>
          </div>
        </div>
      )}
    </>
  )
}

function TorrentRow({
  torrent,
  isSelected,
  onClick,
}: {
  torrent: Torrent
  isSelected: boolean
  onClick: () => void
}) {
  const sizeDisplay =
    torrent.size_mb >= 1024
      ? `${(torrent.size_mb / 1024).toFixed(1)} GB`
      : `${torrent.size_mb} MB`

  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full rounded-lg border p-4 text-left transition-colors hover:bg-muted/50",
        isSelected ? "border-primary bg-primary/5" : "border-border",
        torrent.blacklisted && "opacity-60",
      )}
    >
      <p className="truncate text-sm font-medium">{torrent.title}</p>
      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        {torrent.resolution && <Badge variant="secondary">{torrent.resolution}</Badge>}
        {torrent.codec && <Badge variant="secondary">{torrent.codec}</Badge>}
        {torrent.release_type && <Badge variant="outline">{torrent.release_type}</Badge>}
        {isSelected && (
          <Badge variant="default" className="text-xs">
            Selected
          </Badge>
        )}
        {torrent.blacklisted && (
          <Badge variant="destructive" className="text-xs">
            Blacklisted
          </Badge>
        )}
      </div>
      <div className="mt-2 flex items-center gap-3 text-xs text-muted-foreground">
        <span>{sizeDisplay}</span>
        <span>{torrent.seeders} seeders</span>
        <span>Score {torrent.score.toFixed(1)}</span>
        {torrent.blacklisted_reason && (
          <span className="truncate text-red-400">{torrent.blacklisted_reason}</span>
        )}
      </div>
    </button>
  )
}
