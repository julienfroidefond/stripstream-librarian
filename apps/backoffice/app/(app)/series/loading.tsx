import { PageSkeleton } from "@/app/components/PageSkeleton";

export default function SeriesLoading() {
  return (
    <PageSkeleton
      variant="grid"
      rows={12}
      actions={2}
      tabs={3}
      tabWidths={["w-28", "w-32", "w-24"]}
      filters={8}
      results
    />
  );
}
