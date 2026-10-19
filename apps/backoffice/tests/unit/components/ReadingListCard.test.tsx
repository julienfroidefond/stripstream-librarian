import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ReadingListCard } from "@/app/components/ReadingListCard";
import type { ReadingListDto } from "@/lib/api";

function makeList(overrides: Partial<ReadingListDto> = {}): ReadingListDto {
  return {
    id: "list-1",
    name: "Favourites",
    description: null,
    series_count: 1,
    book_count: 3,
    books_read_count: 1,
    preview_covers: [],
    created_at: "2024-01-01T00:00:00.000Z",
    updated_at: "2024-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function renderCard(overrides: Partial<React.ComponentProps<typeof ReadingListCard>> = {}) {
  render(
    <ReadingListCard
      list={makeList()}
      seriesCountLabel="series"
      booksLabel="books"
      readLabel="read"
      hasActiveUser
      {...overrides}
    />
  );
}

describe("ReadingListCard", () => {
  it("shows user-scoped progress when a reader is selected", () => {
    renderCard();

    expect(screen.getByText("read")).toBeInTheDocument();
    expect(screen.getByText("1 / 3 · 33%")).toBeInTheDocument();
  });

  it("hides user-scoped progress in admin mode", () => {
    renderCard({ hasActiveUser: false });

    expect(screen.queryByText("read")).not.toBeInTheDocument();
    expect(screen.queryByText("1 / 3 · 33%")).not.toBeInTheDocument();
    expect(screen.getByText("books")).toBeInTheDocument();
  });
});
