"use strict";

marked.setOptions({
  gfm: true,
  breaks: false
});

window.renderMarkdown = function (markdown) {
  const content = document.getElementById("content");
  const root = document.scrollingElement;
  const nearBottom = !root ||
    root.scrollHeight - root.scrollTop - root.clientHeight < 80;

  try {
    const html = marked.parse(markdown || "");
    content.innerHTML = DOMPurify.sanitize(html, {
      USE_PROFILES: { html: true },
      FORBID_TAGS: [
        "script", "style", "iframe", "object", "embed", "form",
        "video", "audio"
      ],
      FORBID_ATTR: ["style"]
    });

    renderMathInElement(content, {
      delimiters: [
        { left: "$$", right: "$$", display: true },
        { left: "\\[", right: "\\]", display: true },
        { left: "\\(", right: "\\)", display: false },
        { left: "$", right: "$", display: false }
      ],
      ignoredTags: ["script", "noscript", "style", "textarea", "pre", "code"],
      throwOnError: false,
      strict: "ignore",
      trust: false
    });

    content.querySelectorAll("pre code").forEach((block) => {
      hljs.highlightElement(block);
    });
    content.querySelectorAll("a").forEach((link) => {
      link.rel = "noopener noreferrer";
    });
  } catch (_) {
    content.textContent = markdown || "";
  }

  if (nearBottom) {
    requestAnimationFrame(() => window.scrollTo(0, document.body.scrollHeight));
  }
};
