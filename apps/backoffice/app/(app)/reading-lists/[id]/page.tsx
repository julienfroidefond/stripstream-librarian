import { fetchReadingList } from "@/lib/api";
import { notFound } from "next/navigation";
import { ReadingListDetailClient } from "./ReadingListDetailClient";

export const dynamic = "force-dynamic";

type Props = { params: Promise<{ id: string }> };

export default async function ReadingListDetailPage({ params }: Props) {
  const { id } = await params;
  const list = await fetchReadingList(id).catch(() => null);
  if (!list) notFound();
  return <ReadingListDetailClient list={list} />;
}
