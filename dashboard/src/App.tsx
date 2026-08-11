function App() {
  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <header className="border-b border-zinc-800 px-6 py-4">
        <h1 className="text-xl font-semibold tracking-tight">
          Greplog <span className="font-normal text-zinc-500">Dashboard</span>
        </h1>
      </header>
      <main className="mx-auto grid max-w-6xl gap-6 px-6 py-8 md:grid-cols-2">
        <section className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-5">
          <h2 className="mb-2 font-medium">Live tail</h2>
          <p className="text-sm text-zinc-500">
            Incoming logs will stream here over SSE as the WAL commits them.
          </p>
        </section>
        <section className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-5">
          <h2 className="mb-2 font-medium">Query</h2>
          <p className="text-sm text-zinc-500">
            Run SQL against the DataFusion engine and see results in milliseconds.
          </p>
        </section>
      </main>
    </div>
  )
}

export default App