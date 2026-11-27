// Message text to safe HTML: a small Markdown subset, links, and @mentions.

import DOMPurify from "dompurify";
import { Marked } from "marked";

let knownUsers = new Set();
let me = "";

export function setKnownUsers(users) {
  knownUsers = new Set(users);
}

/** Mentions of this user get the `me` class. */
export function setMe(user) {
  me = user;
}

const mention = {
  name: "mention",
  level: "inline",
  start(src) {
    const i = src.search(/(^|[^\w@])@[a-z0-9_-]{2,24}/i);
    return i < 0 ? undefined : i + (src[i] === "@" ? 0 : 1);
  },
  tokenizer(src) {
    const m = /^@([a-z0-9_-]{2,24})(?![\w-])/i.exec(src);
    if (!m || !knownUsers.has(m[1].toLowerCase())) return undefined;
    return { type: "mention", raw: m[0], user: m[1].toLowerCase() };
  },
  renderer(token) {
    const cls = token.user === me ? "mention me" : "mention";
    return `<span class="${cls}" data-user="${token.user}">@${token.user}</span>`;
  },
};

const md = new Marked({ gfm: true, breaks: true, async: false });
md.use({ extensions: [mention] });
// Headings and images don't belong in chat: show their source text instead.
md.use({
  renderer: {
    heading({ raw }) {
      return `<p>${escapeHtml(raw)}</p>`;
    },
    image({ raw }) {
      return escapeHtml(raw);
    },
    html({ text }) {
      return escapeHtml(text);
    },
  },
});

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noopener noreferrer");
  }
});

const ALLOWED_TAGS = [
  "p", "br", "strong", "em", "del", "code", "pre", "a", "blockquote",
  "ul", "ol", "li", "span", "hr",
];

export function render(text) {
  const html = md.parse(text);
  return DOMPurify.sanitize(html, {
    ALLOWED_TAGS,
    ALLOWED_ATTR: ["href", "class", "data-user", "start"],
    ALLOWED_URI_REGEXP: /^(?:https?:|mailto:)/i,
  });
}

/** Whether `text` mentions `user` (case-insensitive, whole name). */
export function mentions(text, user) {
  const re = new RegExp(`(^|[^\\w@])@${user.replace(/[-]/g, "\\-")}(?![\\w-])`, "i");
  return re.test(text);
}
