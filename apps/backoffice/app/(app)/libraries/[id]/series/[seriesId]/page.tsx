import { redirect } from "next/navigation";

export const dynamic = "force-dynamic";

export default async function OldSeriesDetailPage({
  params,
}: {
  params: Promise<{ id: string; seriesId: string }>;
}) {
  const { seriesId } = await params;
  redirect(`/series/${seriesId}`);
}
