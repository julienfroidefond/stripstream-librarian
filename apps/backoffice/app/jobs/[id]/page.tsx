import { notFound } from "next/navigation";
import Link from "next/link";
import { apiFetch, getMetadataBatchReport, getMetadataBatchResults, MetadataBatchReportDto, MetadataBatchResultDto } from "../../../lib/api";
import {
  Card, CardHeader, CardTitle, CardDescription, CardContent,
  StatusBadge, JobTypeBadge, StatBox, ProgressBar
} from "../../components/ui";

interface JobDetailPageProps {
  params: Promise<{ id: string }>;
}

interface JobDetails {
  id: string;
  library_id: string | null;
  book_id: string | null;
  type: string;
  status: string;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  phase2_started_at: string | null;
  generating_thumbnails_started_at: string | null;
  current_file: string | null;
  progress_percent: number | null;
  processed_files: number | null;
  total_files: number | null;
  stats_json: {
    scanned_files: number;
    indexed_files: number;
    removed_files: number;
    errors: number;
    warnings: number;
  } | null;
  error_opt: string | null;
}

interface JobError {
  id: string;
  file_path: string;
  error_message: string;
  created_at: string;
}

const JOB_TYPE_INFO: Record<string, { label: string; description: string; isThumbnailOnly: boolean }> = {
  rebuild: {
    label: "Indexation incrémentale",
    description: "Scanne les fichiers nouveaux/modifiés, les analyse et génère les miniatures manquantes.",
    isThumbnailOnly: false,
  },
  full_rebuild: {
    label: "Réindexation complète",
    description: "Supprime toutes les données existantes puis effectue un scan complet, une ré-analyse et la génération des miniatures.",
    isThumbnailOnly: false,
  },
  thumbnail_rebuild: {
    label: "Reconstruction des miniatures",
    description: "Génère les miniatures uniquement pour les livres qui n'en ont pas. Les miniatures existantes sont conservées.",
    isThumbnailOnly: true,
  },
  thumbnail_regenerate: {
    label: "Regénération des miniatures",
    description: "Regénère toutes les miniatures depuis zéro, en remplaçant les existantes.",
    isThumbnailOnly: true,
  },
  cbr_to_cbz: {
    label: "Conversion CBR → CBZ",
    description: "Convertit une archive CBR au format ouvert CBZ.",
    isThumbnailOnly: false,
  },
  metadata_batch: {
    label: "Métadonnées en lot",
    description: "Recherche les métadonnées auprès des fournisseurs externes pour toutes les séries de la bibliothèque et applique automatiquement les correspondances à 100% de confiance.",
    isThumbnailOnly: false,
  },
};

async function getJobDetails(jobId: string): Promise<JobDetails | null> {
  try {
    return await apiFetch<JobDetails>(`/index/jobs/${jobId}`);
  } catch {
    return null;
  }
}

async function getJobErrors(jobId: string): Promise<JobError[]> {
  try {
    return await apiFetch<JobError[]>(`/index/jobs/${jobId}/errors`);
  } catch {
    return [];
  }
}

function formatDuration(start: string, end: string | null): string {
  const startDate = new Date(start);
  const endDate = end ? new Date(end) : new Date();
  const diff = endDate.getTime() - startDate.getTime();

  if (diff < 60000) return `${Math.floor(diff / 1000)}s`;
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ${Math.floor((diff % 60000) / 1000)}s`;
  return `${Math.floor(diff / 3600000)}h ${Math.floor((diff % 3600000) / 60000)}m`;
}

function formatSpeed(count: number, durationMs: number): string {
  if (durationMs === 0 || count === 0) return "-";
  return `${(count / (durationMs / 1000)).toFixed(1)}/s`;
}

export default async function JobDetailPage({ params }: JobDetailPageProps) {
  const { id } = await params;
  const [job, errors] = await Promise.all([
    getJobDetails(id),
    getJobErrors(id),
  ]);

  if (!job) {
    notFound();
  }

  const isMetadataBatch = job.type === "metadata_batch";

  // Fetch batch report & results for metadata_batch jobs
  let batchReport: MetadataBatchReportDto | null = null;
  let batchResults: MetadataBatchResultDto[] = [];
  if (isMetadataBatch) {
    [batchReport, batchResults] = await Promise.all([
      getMetadataBatchReport(id).catch(() => null),
      getMetadataBatchResults(id).catch(() => []),
    ]);
  }

  const typeInfo = JOB_TYPE_INFO[job.type] ?? {
    label: job.type,
    description: null,
    isThumbnailOnly: false,
  };

  const durationMs = job.started_at
    ? new Date(job.finished_at || new Date()).getTime() - new Date(job.started_at).getTime()
    : 0;

  const isCompleted = job.status === "success";
  const isFailed = job.status === "failed";
  const isCancelled = job.status === "cancelled";
  const isExtractingPages = job.status === "extracting_pages";
  const isThumbnailPhase = job.status === "generating_thumbnails";
  const isPhase2 = isExtractingPages || isThumbnailPhase;
  const { isThumbnailOnly } = typeInfo;

  // Which label to use for the progress card
  const progressTitle = isMetadataBatch
    ? "Recherche de métadonnées"
    : isThumbnailOnly
      ? "Miniatures"
      : isExtractingPages
        ? "Phase 2 — Extraction des pages"
        : isThumbnailPhase
          ? "Phase 2 — Miniatures"
          : "Phase 1 — Découverte";

  const progressDescription = isMetadataBatch
    ? "Recherche auprès des fournisseurs externes pour chaque série"
    : isThumbnailOnly
      ? undefined
      : isExtractingPages
        ? "Extraction de la première page de chaque archive (nombre de pages + image brute)"
        : isThumbnailPhase
          ? "Génération des miniatures pour les livres analysés"
          : "Scan et indexation des fichiers de la bibliothèque";

  // Speed metric: thumbnail count for thumbnail jobs, scanned files for index jobs
  const speedCount = isThumbnailOnly
    ? (job.processed_files ?? 0)
    : (job.stats_json?.scanned_files ?? 0);

  const showProgressCard =
    (isCompleted || isFailed || job.status === "running" || isPhase2) &&
    (job.total_files != null || !!job.current_file);

  return (
    <>
      <div className="mb-6">
        <Link
          href="/jobs"
          className="inline-flex items-center text-sm text-muted-foreground hover:text-primary transition-colors duration-200"
        >
          <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
          Retour aux tâches
        </Link>
        <h1 className="text-3xl font-bold text-foreground mt-2">Détails de la tâche</h1>
      </div>

      {/* Summary banner — completed */}
      {isCompleted && job.started_at && (
        <div className="mb-6 p-4 rounded-xl bg-success/10 border border-success/30 flex items-start gap-3">
          <svg className="w-5 h-5 text-success mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <div className="text-sm text-success">
            <span className="font-semibold">Terminé en {formatDuration(job.started_at, job.finished_at)}</span>
            {isMetadataBatch && batchReport && (
              <span className="ml-2 text-success/80">
                — {batchReport.auto_matched} auto-associées, {batchReport.already_linked} déjà liées, {batchReport.no_results} aucun résultat, {batchReport.errors} erreurs
              </span>
            )}
            {!isMetadataBatch && job.stats_json && (
              <span className="ml-2 text-success/80">
                — {job.stats_json.scanned_files} scannés, {job.stats_json.indexed_files} indexés
                {job.stats_json.removed_files > 0 && `, ${job.stats_json.removed_files} supprimés`}
                {(job.stats_json.warnings ?? 0) > 0 && `, ${job.stats_json.warnings} avertissements`}
                {job.stats_json.errors > 0 && `, ${job.stats_json.errors} erreurs`}
                {job.total_files != null && job.total_files > 0 && `, ${job.total_files} miniatures`}
              </span>
            )}
            {!isMetadataBatch && !job.stats_json && isThumbnailOnly && job.total_files != null && (
              <span className="ml-2 text-success/80">
                — {job.processed_files ?? job.total_files} miniatures générées
              </span>
            )}
          </div>
        </div>
      )}

      {/* Summary banner — failed */}
      {isFailed && (
        <div className="mb-6 p-4 rounded-xl bg-destructive/10 border border-destructive/30 flex items-start gap-3">
          <svg className="w-5 h-5 text-destructive mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <div className="text-sm text-destructive">
            <span className="font-semibold">Tâche échouée</span>
            {job.started_at && (
              <span className="ml-2 text-destructive/80">après {formatDuration(job.started_at, job.finished_at)}</span>
            )}
            {job.error_opt && (
              <p className="mt-1 text-destructive/70 font-mono text-xs break-all">{job.error_opt}</p>
            )}
          </div>
        </div>
      )}

      {/* Summary banner — cancelled */}
      {isCancelled && (
        <div className="mb-6 p-4 rounded-xl bg-muted border border-border flex items-start gap-3">
          <svg className="w-5 h-5 text-muted-foreground mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
          </svg>
          <span className="text-sm text-muted-foreground">
            <span className="font-semibold">Annulé</span>
            {job.started_at && (
              <span className="ml-2">après {formatDuration(job.started_at, job.finished_at)}</span>
            )}
          </span>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Overview Card */}
        <Card>
          <CardHeader>
            <CardTitle>Aperçu</CardTitle>
            {typeInfo.description && (
              <CardDescription>{typeInfo.description}</CardDescription>
            )}
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between py-2 border-b border-border/60">
              <span className="text-sm text-muted-foreground">ID</span>
              <code className="px-2 py-1 bg-muted rounded font-mono text-sm text-foreground">{job.id}</code>
            </div>
            <div className="flex items-center justify-between py-2 border-b border-border/60">
              <span className="text-sm text-muted-foreground">Type</span>
              <div className="flex items-center gap-2">
                <JobTypeBadge type={job.type} />
                <span className="text-sm text-muted-foreground">{typeInfo.label}</span>
              </div>
            </div>
            <div className="flex items-center justify-between py-2 border-b border-border/60">
              <span className="text-sm text-muted-foreground">Statut</span>
              <StatusBadge status={job.status} />
            </div>
            <div className={`flex items-center justify-between py-2 ${(job.book_id || job.started_at) ? "border-b border-border/60" : ""}`}>
              <span className="text-sm text-muted-foreground">Bibliothèque</span>
              <span className="text-sm text-foreground">{job.library_id || "Toutes les bibliothèques"}</span>
            </div>
            {job.book_id && (
              <div className={`flex items-center justify-between py-2 ${job.started_at ? "border-b border-border/60" : ""}`}>
                <span className="text-sm text-muted-foreground">Livre</span>
                <Link
                  href={`/books/${job.book_id}`}
                  className="text-sm text-primary hover:text-primary/80 font-mono hover:underline"
                >
                  {job.book_id.slice(0, 8)}…
                </Link>
              </div>
            )}
            {job.started_at && (
              <div className="flex items-center justify-between py-2">
                <span className="text-sm text-muted-foreground">Durée</span>
                <span className="text-sm font-semibold text-foreground">
                  {formatDuration(job.started_at, job.finished_at)}
                </span>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Timeline Card */}
        <Card>
          <CardHeader>
            <CardTitle>Chronologie</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="relative">
              {/* Vertical line */}
              <div className="absolute left-[7px] top-2 bottom-2 w-px bg-border" />

              <div className="space-y-5">
                {/* Created */}
                <div className="flex items-start gap-4">
                  <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-muted border-2 border-border shrink-0 z-10" />
                  <div className="flex-1 min-w-0">
                    <span className="text-sm font-medium text-foreground">Créé</span>
                    <p className="text-xs text-muted-foreground">{new Date(job.created_at).toLocaleString()}</p>
                  </div>
                </div>

                {/* Phase 1 start — for index jobs that have two phases */}
                {job.started_at && job.phase2_started_at && (
                  <div className="flex items-start gap-4">
                    <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-primary shrink-0 z-10" />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">Phase 1 — Découverte</span>
                      <p className="text-xs text-muted-foreground">{new Date(job.started_at).toLocaleString()}</p>
                      <p className="text-xs text-primary/80 font-medium mt-0.5">
                        Durée : {formatDuration(job.started_at, job.phase2_started_at)}
                        {job.stats_json && (
                          <span className="text-muted-foreground font-normal ml-1">
                            · {job.stats_json.scanned_files} scannés, {job.stats_json.indexed_files} indexés
                            {job.stats_json.removed_files > 0 && `, ${job.stats_json.removed_files} supprimés`}
                            {(job.stats_json.warnings ?? 0) > 0 && `, ${job.stats_json.warnings} avert.`}
                          </span>
                        )}
                      </p>
                    </div>
                  </div>
                )}

                {/* Phase 2a — Extracting pages (index jobs with phase2) */}
                {job.phase2_started_at && !isThumbnailOnly && (
                  <div className="flex items-start gap-4">
                    <div className={`w-3.5 h-3.5 rounded-full mt-0.5 shrink-0 z-10 ${
                      job.generating_thumbnails_started_at || job.finished_at ? "bg-primary" : "bg-primary animate-pulse"
                    }`} />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">Phase 2a — Extraction des pages</span>
                      <p className="text-xs text-muted-foreground">{new Date(job.phase2_started_at).toLocaleString()}</p>
                      <p className="text-xs text-primary/80 font-medium mt-0.5">
                        Durée : {formatDuration(job.phase2_started_at, job.generating_thumbnails_started_at ?? job.finished_at ?? null)}
                        {!job.generating_thumbnails_started_at && !job.finished_at && isExtractingPages && (
                          <span className="text-muted-foreground font-normal ml-1">· en cours</span>
                        )}
                      </p>
                    </div>
                  </div>
                )}

                {/* Phase 2b — Generating thumbnails */}
                {(job.generating_thumbnails_started_at || (job.phase2_started_at && isThumbnailOnly)) && (
                  <div className="flex items-start gap-4">
                    <div className={`w-3.5 h-3.5 rounded-full mt-0.5 shrink-0 z-10 ${
                      job.finished_at ? "bg-primary" : "bg-primary animate-pulse"
                    }`} />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">
                        {isThumbnailOnly ? "Miniatures" : "Phase 2b — Génération des miniatures"}
                      </span>
                      <p className="text-xs text-muted-foreground">
                        {(job.generating_thumbnails_started_at ? new Date(job.generating_thumbnails_started_at) : job.phase2_started_at ? new Date(job.phase2_started_at) : null)?.toLocaleString()}
                      </p>
                      {(job.generating_thumbnails_started_at || job.finished_at) && (
                        <p className="text-xs text-primary/80 font-medium mt-0.5">
                          Durée : {formatDuration(
                            job.generating_thumbnails_started_at ?? job.phase2_started_at!,
                            job.finished_at ?? null
                          )}
                          {job.total_files != null && job.total_files > 0 && (
                            <span className="text-muted-foreground font-normal ml-1">
                              · {job.processed_files ?? job.total_files} miniatures
                            </span>
                          )}
                        </p>
                      )}
                      {!job.finished_at && isThumbnailPhase && (
                        <span className="text-xs text-muted-foreground">en cours</span>
                      )}
                    </div>
                  </div>
                )}

                {/* Started — for jobs without phase2 (cbr_to_cbz, or no phase yet) */}
                {job.started_at && !job.phase2_started_at && (
                  <div className="flex items-start gap-4">
                    <div className={`w-3.5 h-3.5 rounded-full mt-0.5 shrink-0 z-10 ${
                      job.finished_at ? "bg-primary" : "bg-primary animate-pulse"
                    }`} />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">Démarré</span>
                      <p className="text-xs text-muted-foreground">{new Date(job.started_at).toLocaleString()}</p>
                    </div>
                  </div>
                )}

                {/* Pending — not started yet */}
                {!job.started_at && (
                  <div className="flex items-start gap-4">
                    <div className="w-3.5 h-3.5 rounded-full mt-0.5 bg-warning shrink-0 z-10" />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">En attente de démarrage…</span>
                    </div>
                  </div>
                )}

                {/* Finished */}
                {job.finished_at && (
                  <div className="flex items-start gap-4">
                    <div className={`w-3.5 h-3.5 rounded-full mt-0.5 shrink-0 z-10 ${
                      isCompleted ? "bg-success" : isFailed ? "bg-destructive" : "bg-muted"
                    }`} />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm font-medium text-foreground">
                        {isCompleted ? "Terminé" : isFailed ? "Échoué" : "Annulé"}
                      </span>
                      <p className="text-xs text-muted-foreground">{new Date(job.finished_at).toLocaleString()}</p>
                    </div>
                  </div>
                )}
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Progress Card */}
        {showProgressCard && (
          <Card>
            <CardHeader>
              <CardTitle>{progressTitle}</CardTitle>
              {progressDescription && <CardDescription>{progressDescription}</CardDescription>}
            </CardHeader>
            <CardContent>
              {job.total_files != null && job.total_files > 0 && (
                <>
                  <ProgressBar value={job.progress_percent || 0} showLabel size="lg" className="mb-4" />
                  <div className="grid grid-cols-3 gap-4">
                    <StatBox
                      value={job.processed_files ?? 0}
                      label={isThumbnailOnly || isPhase2 ? "Générés" : "Traités"}
                      variant="primary"
                    />
                    <StatBox value={job.total_files} label="Total" />
                    <StatBox
                      value={Math.max(0, job.total_files - (job.processed_files ?? 0))}
                      label="Restants"
                      variant={isCompleted ? "default" : "warning"}
                    />
                  </div>
                </>
              )}
              {job.current_file && (
                <div className="mt-4 p-3 bg-muted/50 rounded-lg">
                  <span className="text-xs text-muted-foreground uppercase tracking-wide">Fichier en cours</span>
                  <code className="block mt-1 text-xs font-mono text-foreground break-all">{job.current_file}</code>
                </div>
              )}
            </CardContent>
          </Card>
        )}

        {/* Index Statistics — index jobs only */}
        {job.stats_json && !isThumbnailOnly && !isMetadataBatch && (
          <Card>
            <CardHeader>
              <CardTitle>Statistiques d&apos;indexation</CardTitle>
              {job.started_at && (
                <CardDescription>
                  {formatDuration(job.started_at, job.finished_at)}
                  {speedCount > 0 && ` · ${formatSpeed(speedCount, durationMs)} scan rate`}
                </CardDescription>
              )}
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 sm:grid-cols-5 gap-4">
                <StatBox value={job.stats_json.scanned_files} label="Scannés" variant="success" />
                <StatBox value={job.stats_json.indexed_files} label="Indexés" variant="primary" />
                <StatBox value={job.stats_json.removed_files} label="Supprimés" variant="warning" />
                <StatBox value={job.stats_json.warnings ?? 0} label="Avertissements" variant={(job.stats_json.warnings ?? 0) > 0 ? "warning" : "default"} />
                <StatBox value={job.stats_json.errors} label="Erreurs" variant={job.stats_json.errors > 0 ? "error" : "default"} />
              </div>
            </CardContent>
          </Card>
        )}

        {/* Thumbnail statistics — thumbnail-only jobs, completed */}
        {isThumbnailOnly && isCompleted && job.total_files != null && (
          <Card>
            <CardHeader>
              <CardTitle>Statistiques des miniatures</CardTitle>
              {job.started_at && (
                <CardDescription>
                  {formatDuration(job.started_at, job.finished_at)}
                  {speedCount > 0 && ` · ${formatSpeed(speedCount, durationMs)} thumbnails/s`}
                </CardDescription>
              )}
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 gap-4">
                <StatBox value={job.processed_files ?? job.total_files} label="Générés" variant="success" />
                <StatBox value={job.total_files} label="Total" />
              </div>
            </CardContent>
          </Card>
        )}

        {/* Metadata batch report */}
        {isMetadataBatch && batchReport && (
          <Card>
            <CardHeader>
              <CardTitle>Rapport du lot</CardTitle>
              <CardDescription>{batchReport.total_series} séries analysées</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
                <StatBox value={batchReport.auto_matched} label="Auto-associé" variant="success" />
                <StatBox value={batchReport.already_linked} label="Déjà lié" variant="primary" />
                <StatBox value={batchReport.no_results} label="Aucun résultat" />
                <StatBox value={batchReport.too_many_results} label="Trop de résultats" variant="warning" />
                <StatBox value={batchReport.low_confidence} label="Confiance faible" variant="warning" />
                <StatBox value={batchReport.errors} label="Erreurs" variant={batchReport.errors > 0 ? "error" : "default"} />
              </div>
            </CardContent>
          </Card>
        )}

        {/* Metadata batch results */}
        {isMetadataBatch && batchResults.length > 0 && (
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>Résultats par série</CardTitle>
              <CardDescription>{batchResults.length} séries traitées</CardDescription>
            </CardHeader>
            <CardContent className="space-y-2 max-h-[600px] overflow-y-auto">
              {batchResults.map((r) => (
                <div
                  key={r.id}
                  className={`p-3 rounded-lg border ${
                    r.status === "auto_matched" ? "bg-success/10 border-success/20" :
                    r.status === "already_linked" ? "bg-primary/10 border-primary/20" :
                    r.status === "error" ? "bg-destructive/10 border-destructive/20" :
                    "bg-muted/50 border-border/60"
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium text-sm text-foreground truncate">{r.series_name}</span>
                    <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium whitespace-nowrap ${
                      r.status === "auto_matched" ? "bg-success/20 text-success" :
                      r.status === "already_linked" ? "bg-primary/20 text-primary" :
                      r.status === "no_results" ? "bg-muted text-muted-foreground" :
                      r.status === "too_many_results" ? "bg-amber-500/15 text-amber-600" :
                      r.status === "low_confidence" ? "bg-amber-500/15 text-amber-600" :
                      r.status === "error" ? "bg-destructive/20 text-destructive" :
                      "bg-muted text-muted-foreground"
                    }`}>
                      {r.status === "auto_matched" ? "Auto-associé" :
                       r.status === "already_linked" ? "Déjà lié" :
                       r.status === "no_results" ? "Aucun résultat" :
                       r.status === "too_many_results" ? "Trop de résultats" :
                       r.status === "low_confidence" ? "Confiance faible" :
                       r.status === "error" ? "Erreur" :
                       r.status}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 mt-1 text-xs text-muted-foreground">
                    {r.provider_used && (
                      <span>{r.provider_used}{r.fallback_used ? " (secours)" : ""}</span>
                    )}
                    {r.candidates_count > 0 && (
                      <span>{r.candidates_count} candidat{r.candidates_count > 1 ? "s" : ""}</span>
                    )}
                    {r.best_confidence != null && (
                      <span>{Math.round(r.best_confidence * 100)}% confiance</span>
                    )}
                  </div>
                  {r.best_candidate_json && (
                    <p className="text-xs text-muted-foreground mt-1">
                      Correspondance : {(r.best_candidate_json as { title?: string }).title || r.best_candidate_json.toString()}
                    </p>
                  )}
                  {r.error_message && (
                    <p className="text-xs text-destructive/80 mt-1">{r.error_message}</p>
                  )}
                </div>
              ))}
            </CardContent>
          </Card>
        )}

        {/* File errors */}
        {errors.length > 0 && (
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>Erreurs de fichiers ({errors.length})</CardTitle>
              <CardDescription>Erreurs rencontrées lors du traitement des fichiers</CardDescription>
            </CardHeader>
            <CardContent className="space-y-2 max-h-80 overflow-y-auto">
              {errors.map((error) => (
                <div key={error.id} className="p-3 bg-destructive/10 rounded-lg border border-destructive/20">
                  <code className="block text-sm font-mono text-destructive mb-1">{error.file_path}</code>
                  <p className="text-sm text-destructive/80">{error.error_message}</p>
                  <span className="text-xs text-muted-foreground">{new Date(error.created_at).toLocaleString()}</span>
                </div>
              ))}
            </CardContent>
          </Card>
        )}
      </div>
    </>
  );
}
