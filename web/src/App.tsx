import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom"
import { LoginPage } from "@/components/login-form"
import { AppLayout } from "@/components/app-layout"
import { LibraryPage } from "@/pages/library"
import { DiscoverPage } from "@/pages/discover"
import { JobsPage } from "@/pages/jobs"
import { useSession } from "@/lib/auth"

function App() {
  const { data: session, isPending } = useSession()

  if (isPending) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="size-6 animate-spin rounded-full border-2 border-muted-foreground border-t-transparent" />
      </div>
    )
  }

  if (!session) {
    return (
      <BrowserRouter basename="/web">
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route path="*" element={<Navigate to="/login" replace />} />
        </Routes>
      </BrowserRouter>
    )
  }

  return (
    <BrowserRouter basename="/web">
      <Routes>
        <Route element={<AppLayout session={session} />}>
          <Route index element={<LibraryPage />} />
          <Route path="/discover" element={<DiscoverPage />} />
          <Route path="/discover/:mediaType/:id" element={<DiscoverPage />} />
          <Route path="/jobs" element={<JobsPage />} />
        </Route>
        <Route path="/login" element={<Navigate to="/" replace />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}

export default App
