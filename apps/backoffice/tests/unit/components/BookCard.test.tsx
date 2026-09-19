import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BookCard, BooksGrid, BooksGridWithMissing, EmptyState } from "@/app/components/BookCard";
import type { BookDto } from "@/lib/api";

type CardBook = BookDto & { coverUrl?: string };

function makeBook(overrides: Partial<CardBook> = {}): CardBook {
  return {
    id: "b1",
    library_id: "l1",
    kind: "cbz",
    format: "cbz",
    title: "Berserk vol. 1",
    author: "Kentaro Miura",
    authors: [],
    series: "Berserk",
    volume: 1,
    volume_type: "regular",
    language: "fr",
    page_count: 200,
    file_path: null,
    file_format: null,
    file_parse_status: null,
    updated_at: "2024-01-01",
    reading_status: "unread",
    reading_current_page: null,
    reading_last_read_at: null,
    series_id: "s1",
    summary: null,
    isbn: null,
    publish_date: null,
    ...overrides,
  };
}

describe("BookCard", () => {
  it("renders the title, author, series and volume, and links to the book", () => {
    render(<BookCard book={makeBook()} />);

    const link = screen.getByRole("link");
    expect(link).toHaveAttribute("href", "/books/b1");
    expect(screen.getByRole("heading", { name: "Berserk vol. 1" })).toBeInTheDocument();
    expect(screen.getByText("Kentaro Miura")).toBeInTheDocument();
    expect(screen.getByText("#1")).toBeInTheDocument();
  });

  it("uses the provided cover URL or falls back to the thumbnail endpoint", () => {
    const { container, rerender } = render(<BookCard book={makeBook()} />);
    expect(container.querySelector("img")).toHaveAttribute("src", "/api/books/b1/thumbnail");

    rerender(<BookCard book={makeBook({ coverUrl: "/covers/b1.webp" })} />);
    expect(container.querySelector("img")).toHaveAttribute("src", "/covers/b1.webp");
  });

  it("shows the reading status overlay and dims a read book", () => {
    const { container } = render(<BookCard book={makeBook({ reading_status: "read" })} />);

    expect(screen.getByText("status.read")).toBeInTheDocument();
    fireEvent.load(container.querySelector("img") as HTMLImageElement);
    expect(container.querySelector("img")?.className).toContain("opacity-40");
  });

  it("lets the readingStatus prop override the book status", () => {
    render(<BookCard book={makeBook({ reading_status: "read" })} readingStatus="reading" />);
    expect(screen.getByText("status.reading")).toBeInTheDocument();
    expect(screen.queryByText("status.read")).toBeNull();
  });

  it("falls back to the kind when the format is missing", () => {
    render(<BookCard book={makeBook({ format: null, kind: "pdf" })} />);
    expect(screen.getByText("pdf")).toBeInTheDocument();
  });

  it("renders the language badge when present", () => {
    const { rerender } = render(<BookCard book={makeBook({ language: "fr" })} />);
    expect(screen.getByText("fr")).toBeInTheDocument();

    rerender(<BookCard book={makeBook({ language: null })} />);
    expect(screen.queryByText("fr")).toBeNull();
  });

  it("renders a placeholder icon when the cover fails to load", () => {
    const { container } = render(<BookCard book={makeBook()} />);
    fireEvent.error(container.querySelector("img") as HTMLImageElement);
    expect(container.querySelector("img")).toBeNull();
  });
});

describe("BookCard compact", () => {
  it("renders the volume-type labels", () => {
    const { rerender, container } = render(
      <BookCard compact book={makeBook({ volume_type: "hs", volume: 2 })} />
    );
    expect(screen.getByText("HS #2")).toBeInTheDocument();

    rerender(<BookCard compact book={makeBook({ volume_type: "integral", volume: 3 })} />);
    expect(screen.getByText("INT #3")).toBeInTheDocument();

    rerender(<BookCard compact book={makeBook({ volume_type: "oneshot" })} />);
    expect(screen.getByText("One-shot")).toBeInTheDocument();

    rerender(<BookCard compact book={makeBook({ volume_type: "regular", volume: 5 })} />);
    expect(container.querySelector("h3")).toHaveTextContent("Berserk vol. 1");
  });

  it("hides the read button when there is no active user", () => {
    const { rerender } = render(<BookCard compact book={makeBook()} hasActiveUser={false} />);
    expect(screen.queryByRole("button")).toBeNull();

    rerender(<BookCard compact book={makeBook()} hasActiveUser />);
    expect(screen.getByRole("button")).toBeInTheDocument();
  });
});

describe("BooksGrid", () => {
  it("renders one card per book", () => {
    render(
      <BooksGrid
        books={[makeBook({ id: "a", title: "A" }), makeBook({ id: "b", title: "B" })]}
        hasActiveUser={false}
      />
    );

    expect(screen.getAllByRole("link")).toHaveLength(2);
    expect(screen.getByRole("heading", { name: "A" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "B" })).toBeInTheDocument();
  });
});

describe("BooksGridWithMissing", () => {
  it("delegates to BooksGrid when showMissing is false", () => {
    const { container } = render(
      <BooksGridWithMissing
        books={[makeBook({ id: "a", title: "A" })]}
        missingBooks={[{ title: "Missing", volume_number: 9, cover_url: null }]}
        showMissing={false}
        hasActiveUser={false}
      />
    );

    expect(screen.queryByText("Missing")).toBeNull();
    expect(container.querySelectorAll(".grid > *")).toHaveLength(1);
  });

  it("merges owned and missing books sorted by volume", () => {
    render(
      <BooksGridWithMissing
        books={[makeBook({ id: "owned-2", title: "Owned 2", volume: 2 })]}
        missingBooks={[
          { title: "Missing 1", volume_number: 1, cover_url: null },
          { title: "Missing 3", volume_number: 3, cover_url: null, href: "/books/missing?series=1" },
        ]}
        showMissing
        compact
        hasActiveUser={false}
      />
    );

    const headings = screen.getAllByRole("heading").map((h) => h.textContent);
    expect(headings).toEqual(["Missing 1", "Owned 2", "Missing 3"]);
  });

  it("filters missing volumes already owned by volume or by title", () => {
    render(
      <BooksGridWithMissing
        books={[
          makeBook({ id: "owned-2", title: "Owned 2", volume: 2 }),
          makeBook({ id: "titled", title: "Series Tome 4", volume: null }),
        ]}
        missingBooks={[
          { title: "Missing 2", volume_number: 2, cover_url: null },
          { title: "Missing 4", volume_number: 4, cover_url: null },
          { title: "Missing unknown", volume_number: null, cover_url: null },
        ]}
        showMissing
        compact
        hasActiveUser={false}
      />
    );

    expect(screen.queryByText("Missing 2")).toBeNull();
    expect(screen.queryByText("Missing 4")).toBeNull();
    expect(screen.getByText("Missing unknown")).toBeInTheDocument();
  });

  it("links missing books with an href and shows a cover image when available", () => {
    const { container } = render(
      <BooksGridWithMissing
        books={[]}
        missingBooks={[
          {
            title: "Missing 3",
            volume_number: 3,
            cover_url: "https://covers/3.jpg",
            href: "/books/missing?series=1",
          },
        ]}
        showMissing
        compact
        hasActiveUser={false}
      />
    );

    const link = screen.getByRole("link");
    expect(link).toHaveAttribute("href", "/books/missing?series=1");
    expect(container.querySelector("img")).toHaveAttribute("src", "https://covers/3.jpg");
    expect(screen.getByText("status.missing")).toBeInTheDocument();
  });
});

describe("EmptyState", () => {
  it("renders the message with the default icon and a custom icon", () => {
    const { container, rerender } = render(<EmptyState message="Nothing here" />);
    expect(screen.getByText("Nothing here")).toBeInTheDocument();
    expect(container.querySelector("svg")).toBeInTheDocument();

    rerender(<EmptyState message="Custom" icon={<span data-testid="custom-icon" />} />);
    expect(screen.getByTestId("custom-icon")).toBeInTheDocument();
  });
});
