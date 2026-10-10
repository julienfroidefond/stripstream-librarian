import { NextRequest, NextResponse } from "next/server";
import { apiFetch, updateSetting } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ key: string }> }) => {
  const { key } = await params;
  const data = await apiFetch<unknown>(`/settings/${key}`);
  return NextResponse.json(data);
}, { message: "Failed to fetch setting" });

export const POST = withRoute(async (
  request: NextRequest,
  { params }: { params: Promise<{ key: string }> }) => {
  const { key } = await params;
  const { value } = await request.json();
  const data = await updateSetting(key, value);
  return NextResponse.json(data);
}, { message: "Failed to update setting" });
