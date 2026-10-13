import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BookPreview } from "@/app/components/BookPreview";

describe("BookPreview", () => {
  it("renders up to five page thumbnails", () => {
    render(<BookPreview bookId="book-1" pageCount={3} />);

    const images = screen.getAllByRole("img");
    expect(images).toHaveLength(3);
    expect(images[0]).toHaveAttribute(
      "src",
      "/api/books/book-1/pages/1?format=webp&width=600&quality=80"
    );
  });

  it("shows an unavailable placeholder when a page fails to load", () => {
    render(<BookPreview bookId="book-1" pageCount={2} />);

    fireEvent.error(screen.getAllByRole("img")[0]);

    expect(screen.getByText("bookPreview.pageUnavailable")).toBeInTheDocument();
    // The broken image is replaced, only the second page keeps an <img>.
    expect(screen.getAllByRole("img")).toHaveLength(1);
  });
});
