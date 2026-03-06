export default function DashboardPage() {
  return (
    <div className="max-w-5xl mx-auto">
      <div className="text-center mb-12">
        <h1 className="text-4xl font-bold tracking-tight mb-4 text-foreground">
          StripStream Backoffice
        </h1>
        <p className="text-lg text-muted-foreground max-w-2xl mx-auto">
          Manage libraries, indexing jobs, and API tokens from a modern admin interface.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        {/* Libraries Card */}
        <a 
          href="/libraries" 
          className="group p-6 bg-card/80 backdrop-blur-sm rounded-xl border border-border/50 shadow-sm hover:shadow-lg hover:-translate-y-1 hover:bg-card/95 hover:border-border/80 transition-all duration-300"
        >
          <div className="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center mb-4 group-hover:bg-primary transition-colors duration-200">
            <svg className="w-6 h-6 text-primary group-hover:text-primary-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          </div>
          <h2 className="text-xl font-semibold mb-2 text-foreground">Libraries</h2>
          <p className="text-muted-foreground text-sm leading-relaxed">Manage your comic libraries and folders</p>
        </a>

        {/* Books Card */}
        <a 
          href="/books" 
          className="group p-6 bg-card/80 backdrop-blur-sm rounded-xl border border-border/50 shadow-sm hover:shadow-lg hover:-translate-y-1 hover:bg-card/95 hover:border-border/80 transition-all duration-300"
        >
          <div className="w-12 h-12 bg-success/10 rounded-lg flex items-center justify-center mb-4 group-hover:bg-success transition-colors duration-200">
            <svg className="w-6 h-6 text-success group-hover:text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
            </svg>
          </div>
          <h2 className="text-xl font-semibold mb-2 text-foreground">Books</h2>
          <p className="text-muted-foreground text-sm leading-relaxed">Browse and search your comic collection</p>
        </a>

        {/* Jobs Card */}
        <a 
          href="/jobs" 
          className="group p-6 bg-card/80 backdrop-blur-sm rounded-xl border border-border/50 shadow-sm hover:shadow-lg hover:-translate-y-1 hover:bg-card/95 hover:border-border/80 transition-all duration-300"
        >
          <div className="w-12 h-12 bg-warning/10 rounded-lg flex items-center justify-center mb-4 group-hover:bg-warning transition-colors duration-200">
            <svg className="w-6 h-6 text-warning group-hover:text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </div>
          <h2 className="text-xl font-semibold mb-2 text-foreground">Jobs</h2>
          <p className="text-muted-foreground text-sm leading-relaxed">Monitor indexing jobs and progress</p>
        </a>

        {/* Tokens Card */}
        <a 
          href="/tokens" 
          className="group p-6 bg-card/80 backdrop-blur-sm rounded-xl border border-border/50 shadow-sm hover:shadow-lg hover:-translate-y-1 hover:bg-card/95 hover:border-border/80 transition-all duration-300"
        >
          <div className="w-12 h-12 bg-destructive/10 rounded-lg flex items-center justify-center mb-4 group-hover:bg-destructive transition-colors duration-200">
            <svg className="w-6 h-6 text-destructive group-hover:text-destructive-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
            </svg>
          </div>
          <h2 className="text-xl font-semibold mb-2 text-foreground">Tokens</h2>
          <p className="text-muted-foreground text-sm leading-relaxed">Manage API authentication tokens</p>
        </a>
      </div>

      <div className="mt-12 p-6 bg-primary/5 backdrop-blur-sm rounded-xl border border-primary/20 hover:bg-primary/8 hover:border-primary/30 transition-all duration-300">
        <h2 className="text-lg font-semibold mb-2 text-primary">Getting Started</h2>
        <p className="text-muted-foreground leading-relaxed">
          Start by creating a library from your comic folders, then trigger an index job to scan your collection.
        </p>
      </div>
    </div>
  );
}
