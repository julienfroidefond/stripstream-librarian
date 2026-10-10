import { NextRequest, NextResponse } from "next/server";
import { convertBook } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(
  async (_request: NextRequest, { params }: { params: Promise<{ bookId: string }> }) => {
    const { bookId } = await params;
    const data = await convertBook(bookId);
    return NextResponse.json(data);
  },
  {
    fallback: "Failed to start conversion",
    statusFromError: (_error, message) => (message.includes("409") ? 409 : 500),
  }
);
