import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  SeriesUserRatings,
  SeriesUserReading,
} from "@/app/components/SeriesUserActivity";
import type { SeriesUserActivityDto } from "@/lib/api";

function makeActivity(
  overrides: Partial<SeriesUserActivityDto> = {}
): SeriesUserActivityDto {
  return {
    user_id: "u1",
    username: "Alice",
    books_read: 2,
    books_reading: 1,
    books_total: 8,
    last_read_at: "2026-10-19",
    rating: null,
    ...overrides,
  };
}

describe("SeriesUserRatings", () => {
  it("renders every reader's rating and skips unrated readers", () => {
    render(
      <SeriesUserRatings
        items={[
          makeActivity({ user_id: "u1", username: "Alice", rating: 8 }),
          makeActivity({ user_id: "u2", username: "Bob", rating: null }),
          makeActivity({ user_id: "u3", username: "Carol", rating: 7 }),
        ]}
      />
    );

    expect(screen.getByText("series.userRatings")).toBeInTheDocument();
    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.queryByText("Bob")).toBeNull();
    expect(screen.getByText("Carol")).toBeInTheDocument();
    expect(screen.getByText("4.0")).toBeInTheDocument();
    expect(screen.getByText("3.5")).toBeInTheDocument();
  });

  it("renders nothing when no reader rated the series", () => {
    const { container } = render(
      <SeriesUserRatings items={[makeActivity({ rating: null })]} />
    );

    expect(container).toBeEmptyDOMElement();
  });
});

describe("SeriesUserReading", () => {
  it("renders read, in-progress and total counts per reader", () => {
    render(
      <SeriesUserReading
        items={[
          makeActivity({ user_id: "u1", username: "Alice", books_read: 2, books_reading: 1, books_total: 8 }),
          makeActivity({ user_id: "u2", username: "Bob", books_read: 0, books_reading: 0 }),
        ]}
      />
    );

    expect(screen.getByText("series.userReading")).toBeInTheDocument();
    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("+1")).toBeInTheDocument();
    expect(screen.getByText("/8")).toBeInTheDocument();
    expect(screen.queryByText("Bob")).toBeNull();
  });

  it("renders nothing when no reader started the series", () => {
    const { container } = render(
      <SeriesUserReading
        items={[makeActivity({ books_read: 0, books_reading: 0 })]}
      />
    );

    expect(container).toBeEmptyDOMElement();
  });
});
