import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DownloadBookFileButton } from "@/app/components/DownloadBookFileButton";

describe("DownloadBookFileButton", () => {
  it("renders a download link to the book's original file", () => {
    render(<DownloadBookFileButton bookId="b1" />);

    const link = screen.getByRole("menuitem");
    expect(link.tagName).toBe("A");
    expect(link).toHaveAttribute("href", "/api/books/b1/file");
    expect(link).toHaveAttribute("download");
    expect(link).toHaveTextContent("bookDetail.downloadOriginal");
  });
});
