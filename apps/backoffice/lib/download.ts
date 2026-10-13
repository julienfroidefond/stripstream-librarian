/**
 * iOS standalone web apps (added to the home screen) cannot download a file by
 * navigating to it: the web view is trapped on a blank document with no way back
 * (WebKit bug, see https://github.com/tobias-bischoff/ios-pwa-download-reproduction).
 * The workaround is to hand the file to the native share sheet instead.
 */

export function isIosStandalone(): boolean {
  if (typeof navigator === "undefined") return false;
  return (navigator as Navigator & { standalone?: boolean }).standalone === true;
}

/** Extract the filename from a `Content-Disposition` header (RFC 6266 / 5987). */
export function filenameFromContentDisposition(header: string | null): string | null {
  if (!header) return null;

  const encoded = /filename\*=UTF-8''([^;]+)/i.exec(header);
  if (encoded) {
    try {
      return decodeURIComponent(encoded[1]);
    } catch {
      return null;
    }
  }

  const plain = /filename="([^"]+)"/i.exec(header);
  return plain ? plain[1] : null;
}

/** Fetch the book file as a `File` so it can be passed to `navigator.share`. */
export async function fetchSharedFile(url: string, fallbackName = "book"): Promise<File> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`download failed (${response.status})`);
  }

  const blob = await response.blob();
  const filename =
    filenameFromContentDisposition(response.headers.get("content-disposition")) ?? fallbackName;
  return new File([blob], filename, { type: blob.type || "application/octet-stream" });
}
