import { PageSkeleton } from "@/app/components/PageSkeleton";

export default function BooksLoading() {
  return <PageSkeleton variant="grid" rows={12} />;
}
