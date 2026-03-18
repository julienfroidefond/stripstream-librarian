import sanitizeHtml from "sanitize-html";
import React from "react";

interface SafeHtmlProps {
  html: string;
  className?: string;
  as?: "div" | "p" | "span";
}

const sanitizeOptions: sanitizeHtml.IOptions = {
  allowedTags: [
    "b", "i", "em", "strong", "a", "p", "br", "ul", "ol", "li",
    "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "span",
  ],
  allowedAttributes: {
    a: ["href", "target", "rel"],
    span: ["class"],
  },
  transformTags: {
    a: sanitizeHtml.simpleTransform("a", { target: "_blank", rel: "noopener noreferrer" }),
  },
};

export function SafeHtml({ html, className, as: tag = "div" }: SafeHtmlProps) {
  return React.createElement(tag, {
    className,
    dangerouslySetInnerHTML: { __html: sanitizeHtml(html, sanitizeOptions) },
  });
}
