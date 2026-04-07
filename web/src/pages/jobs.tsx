import * as React from "react"
import {
  ListTodo,
  Clock,
  Search,
  Zap,
  CheckCircle2,
  XCircle,
  Download,
  Save,
  Trash2,
  Loader2,
  ChevronLeft,
  ChevronRight,
} from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { fetchJobs, deleteJob } from "@/lib/api"
import { useTMDBMeta } from "@/lib/hooks"
import type { Job, JobStage } from "@/lib/types"
import { cn } from "@/lib/utils"

const POSTER_BASE = "https://image.tmdb.org/t/p/w92"
const POLL_INTERVAL = 5000

type FilterValue = "all" | JobStage

interface StageConfig {
  label: string
  icon: typeof Clock
  variant: "default" | "info" | "purple" | "warning" | "success" | "destructive" | "secondary"
  animated?: boolean
}

const STAGE_CONFIG: Record<JobStage, StageConfig> = {
  pending: { label: "Pending", icon: Clock, variant: "secondary" },
  indexing: { label: "Indexing", icon: Search, variant: "info", animated: true },
  downloading: { label: "Downloading", icon: Download, variant: "info", animated: true },
  sterilizing: { label: "Sterilizing", icon: Zap, variant: "warning", animated: true },
  saving: { label: "Saving", icon: Save, variant: "purple", animated: true },
  cleanup: { label: "Cleanup", icon: Trash2, variant: "secondary", animated: true },
  finished: { label: "Completed", icon: CheckCircle2, variant: "success" },
  failed: { label: "Failed", icon: XCircle, variant: "destructive" },
}

const FILTERS: { value: FilterValue; label: string }[] = [
  { value: "all", label: "All Jobs" },
  { value: "pending", label: "Pending" },
  { value: "indexing", label: "Indexing" },
  { value: "downloading", label: "Downloading" },
  { value: "sterilizing", label: "Sterilizing" },
  { value: "finished", label: "Completed" },
  { value: "failed", label: "Failed" },
]

export function JobsPage() {
  const [jobs, setJobs] = React.useState<Job[]>([])
  const [loading, setLoading] = React.useState(true)
  const [page, setPage] = React.useState(1)
  const [totalPages, setTotalPages] = React.useState(1)
  const [total, setTotal] = React.useState(0)
  const [filter, setFilter] = React.useState<FilterValue>("all")
  const [deletingId, setDeletingId] = React.useState<string | null>(null)
  const { meta, loadingIds, fetchMeta } = useTMDBMeta()

  const loadJobs = React.useCallback((p: number) => {
    fetchJobs(p, 50).then((data) => {
      setJobs(data.results)
      setTotalPages(data.total_pages)
      setTotal(data.total)
      setLoading(false)
    }).catch(() => setLoading(false))
  }, [])

  React.useEffect(() => {
    setLoading(true)
    loadJobs(page)
  }, [page, loadJobs])

  // Poll for updates only while there are active (non-terminal) jobs
  const hasActiveJobs = jobs.some((j) => j.current_stage !== "finished" && j.current_stage !== "failed")
  React.useEffect(() => {
    if (!hasActiveJobs) return
    const interval = setInterval(() => loadJobs(page), POLL_INTERVAL)
    return () => clearInterval(interval)
  }, [page, loadJobs, hasActiveJobs])

  // Fetch TMDB metadata for jobs missing title/poster
  React.useEffect(() => {
    const needsMeta = jobs.filter((j) => !j.title)
    const uniqueIds = [...new Set(needsMeta.map((j) => j.imdb_id))]
    uniqueIds.forEach(fetchMeta)
  }, [jobs, fetchMeta])

  async function handleDelete(id: string) {
    setDeletingId(id)
    try {
      await deleteJob(id)
      setJobs((prev) => prev.filter((j) => j.id !== id))
    } catch {
      // Silently fail - job may have already been deleted
    }
    setDeletingId(null)
  }

  const filteredJobs = filter === "all"
    ? jobs
    : jobs.filter((j) => j.current_stage === filter)

  const activeCount = jobs.filter((j) =>
    !["finished", "failed"].includes(j.current_stage) && j.current_stage !== "pending"
  ).length
  const completedCount = jobs.filter((j) => j.current_stage === "finished").length
  const failedCount = jobs.filter((j) => j.current_stage === "failed").length

  return (
    <div className="mx-auto max-w-[1600px] px-4 py-6 lg:px-6 lg:py-8">

      {/* Stats */}
      <div className="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-4 lg:gap-4">
        <StatCard label="Total" value={total} icon={ListTodo} color="text-foreground" />
        <StatCard label="In Progress" value={activeCount} icon={Zap} color="text-blue-500" />
        <StatCard label="Completed" value={completedCount} icon={CheckCircle2} color="text-emerald-500" />
        <StatCard label="Failed" value={failedCount} icon={XCircle} color="text-destructive" />
      </div>

      {/* Filters */}
      <div className="mb-4 flex flex-wrap gap-1.5 lg:gap-2">
        {FILTERS.map(({ value, label }) => (
          <button
            key={value}
            onClick={() => setFilter(value)}
            className={cn(
              "rounded-lg px-3 py-1.5 text-xs font-medium transition-colors lg:px-4 lg:py-2 lg:text-sm",
              filter === value
                ? "bg-ring text-white"
                : "bg-muted text-muted-foreground hover:text-foreground",
            )}
          >
            {label}
            {value === "all" && ` (${jobs.length})`}
          </button>
        ))}
      </div>

      {/* Job List */}
      {loading ? (
        <div className="space-y-3">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-16 rounded-lg" />
          ))}
        </div>
      ) : filteredJobs.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <ListTodo className="mb-4 size-12 text-muted-foreground/50" />
          <h2 className="mb-1 text-lg font-medium">No jobs</h2>
          <p className="text-sm text-muted-foreground">
            {filter === "all"
              ? "Search for something to start a job."
              : "No jobs match this filter."}
          </p>
        </div>
      ) : (
        <>
          {/* Desktop table */}
          <div className="hidden md:block">
            <div className="rounded-xl border">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b bg-muted/30">
                    <th className="px-4 py-2.5 text-left font-medium text-muted-foreground">Media</th>
                    <th className="px-4 py-2.5 text-left font-medium text-muted-foreground">Status</th>
                    <th className="px-4 py-2.5 text-left font-medium text-muted-foreground">Message</th>
                    <th className="px-4 py-2.5 text-left font-medium text-muted-foreground">Updated</th>
                    <th className="w-10" />
                  </tr>
                </thead>
                <tbody>
                  {filteredJobs.map((job) => (
                    <JobRow
                      key={job.id}
                      job={job}
                      meta={meta[job.imdb_id]}
                      metaLoading={!job.title && loadingIds.has(job.imdb_id)}
                      deleting={deletingId === job.id}
                      onDelete={() => handleDelete(job.id)}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* Mobile cards */}
          <div className="space-y-3 md:hidden">
            {filteredJobs.map((job) => (
              <JobCard
                key={job.id}
                job={job}
                meta={meta[job.imdb_id]}
                metaLoading={!job.title && loadingIds.has(job.imdb_id)}
                deleting={deletingId === job.id}
                onDelete={() => handleDelete(job.id)}
              />
            ))}
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
    </div>
  )
}

// ---- Scoped components ----

interface StatCardProps {
  label: string
  value: number
  icon: typeof Clock
  color: string
}

function StatCard({ label, value, icon: Icon, color }: StatCardProps) {
  return (
    <div className="rounded-xl border bg-card p-4 lg:p-5">
      <div className="flex items-center gap-2">
        <Icon className={cn("size-4 lg:size-5", color)} />
        <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground lg:text-sm">
          {label}
        </span>
      </div>
      <p className="mt-1 text-2xl font-bold lg:text-3xl">{value}</p>
    </div>
  )
}

interface JobItemProps {
  job: Job
  meta?: { title: string; posterPath: string | null } | undefined
  metaLoading: boolean
  deleting: boolean
  onDelete: () => void
}

function JobRow({ job, meta, metaLoading, deleting, onDelete }: JobItemProps) {
  const config = STAGE_CONFIG[job.current_stage]
  const Icon = config.icon
  const canDelete = job.current_stage === "finished" || job.current_stage === "failed"
  const title = job.title || meta?.title
  const poster = job.poster_path || meta?.posterPath

  return (
    <tr className={cn(
      "border-b last:border-0 transition-colors group",
      job.current_stage === "failed" && "bg-destructive/5",
    )}>
      <td className="px-4 py-3">
        <div className="flex items-center gap-3">
          {metaLoading ? (
            <Skeleton className="size-10 rounded" />
          ) : poster ? (
            <img
              src={`${POSTER_BASE}${poster}`}
              alt={title}
              className="size-10 rounded object-cover"
            />
          ) : (
            <div className="size-10 rounded bg-muted" />
          )}
          <div className="min-w-0">
            {metaLoading ? (
              <>
                <Skeleton className="h-4 w-32 mb-1" />
                <Skeleton className="h-3 w-20" />
              </>
            ) : (
              <>
                <p className="truncate font-medium">{title || job.imdb_id}</p>
                <p className="text-xs text-muted-foreground">
                  {job.imdb_id}
                  {job.season !== null && ` - S${String(job.season).padStart(2, "0")}`}
                </p>
              </>
            )}
          </div>
        </div>
      </td>
      <td className="px-4 py-3">
        <Badge variant={config.variant} className="gap-1">
          <Icon className={cn("size-3", config.animated && "animate-pulse")} />
          {config.label}
        </Badge>
      </td>
      <td className="px-4 py-3">
        <p className="max-w-xs truncate text-sm text-muted-foreground">
          {job.last_log || "-"}
        </p>
      </td>
      <td className="px-4 py-3 text-xs text-muted-foreground">
        {formatRelativeTime(job.updated_at)}
      </td>
      <td className="px-4 py-3">
        {canDelete && (
          <Button
            variant="ghost"
            size="icon-xs"
            className="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive"
            onClick={onDelete}
            disabled={deleting}
          >
            {deleting ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <Trash2 className="size-3" />
            )}
          </Button>
        )}
      </td>
    </tr>
  )
}

function JobCard({ job, meta, metaLoading, deleting, onDelete }: JobItemProps) {
  const config = STAGE_CONFIG[job.current_stage]
  const Icon = config.icon
  const canDelete = job.current_stage === "finished" || job.current_stage === "failed"
  const title = job.title || meta?.title
  const poster = job.poster_path || meta?.posterPath

  return (
    <div className={cn(
      "rounded-xl border bg-card p-4",
      job.current_stage === "failed" && "border-destructive/30 bg-destructive/5",
    )}>
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-3 min-w-0">
          {metaLoading ? (
            <Skeleton className="size-12 shrink-0 rounded" />
          ) : poster ? (
            <img
              src={`${POSTER_BASE}${poster}`}
              alt={title}
              className="size-12 rounded object-cover"
            />
          ) : (
            <div className="size-12 shrink-0 rounded bg-muted" />
          )}
          <div className="min-w-0">
            {metaLoading ? (
              <>
                <Skeleton className="h-4 w-28 mb-1" />
                <Skeleton className="h-3 w-16" />
              </>
            ) : (
              <>
                <p className="truncate font-medium text-sm">{title || job.imdb_id}</p>
                <p className="text-xs text-muted-foreground">
                  {job.imdb_id}
                  {job.season !== null && ` - S${String(job.season).padStart(2, "0")}`}
                </p>
              </>
            )}
          </div>
        </div>
        <Badge variant={config.variant} className="shrink-0 gap-1">
          <Icon className={cn("size-3", config.animated && "animate-pulse")} />
          {config.label}
        </Badge>
      </div>

      {job.last_log && (
        <p className="mt-2 truncate text-xs text-muted-foreground">{job.last_log}</p>
      )}

      <div className="mt-2 flex items-center justify-between">
        <span className="text-xs text-muted-foreground">
          {formatRelativeTime(job.updated_at)}
        </span>
        {canDelete && (
          <Button
            variant="ghost"
            size="icon-xs"
            className="text-muted-foreground hover:text-destructive"
            onClick={onDelete}
            disabled={deleting}
          >
            {deleting ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <Trash2 className="size-3" />
            )}
          </Button>
        )}
      </div>
    </div>
  )
}

function formatRelativeTime(dateStr: string): string {
  const now = Date.now()
  const then = new Date(dateStr).getTime()
  const diff = now - then

  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return "just now"

  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`

  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`

  const days = Math.floor(hours / 24)
  return `${days}d ago`
}
