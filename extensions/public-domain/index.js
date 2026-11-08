// Public Domain Animation, a real source for the Kaminari JS host.
// It reads archive.org's public domain cartoon collection.

(function () {
  const BASE = "https://archive.org";

  function text(value) {
    return value == null ? "" : String(value);
  }

  function first(value) {
    return Array.isArray(value) ? value[0] : value;
  }

  function yearOf(doc) {
    const year = parseInt(first(doc.year), 10);
    return Number.isFinite(year) ? year : undefined;
  }

  function clean(value) {
    return text(value)
      .replace(/<br\s*\/?>/gi, " ")
      .replace(/<[^>]+>/g, " ")
      .replace(/&quot;/g, '"')
      .replace(/&#0?39;/g, "'")
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/\s+/g, " ")
      .trim();
  }

  function durationOf(value) {
    const raw = first(value);
    if (!raw) return undefined;
    const str = String(raw);
    if (str.includes(":")) {
      const parts = str.split(":").map((part) => parseInt(part, 10) || 0);
      return parts.length === 3
        ? parts[0] * 3600 + parts[1] * 60 + parts[2]
        : parts[0] * 60 + parts[1];
    }
    const seconds = parseInt(str, 10);
    return Number.isFinite(seconds) ? seconds : undefined;
  }

  function search(query, page) {
    const textQuery = query && String(query).trim() ? `title:(${String(query).trim()}) AND ` : "title:(Popeye) AND ";
    const url =
      `${BASE}/advancedsearch.php?q=${encodeURIComponent(textQuery + "collection:animationandcartoons AND mediatype:movies")}` +
      `&fl[]=identifier&fl[]=title&fl[]=year&fl[]=description&rows=24&page=${page || 1}&output=json`;
    const res = net.fetch(url);
    if (res.status !== 200) {
      throw { code: "HTTP", message: `archive.org answered ${res.status}`, retryable: true };
    }
    const data = JSON.parse(res.body);
    const docs = (data.response && data.response.docs) || [];
    return {
      items: docs.map((doc) => ({
        id: String(doc.identifier),
        title: text(first(doc.title)) || String(doc.identifier),
        cover: `${BASE}/services/img/${encodeURIComponent(doc.identifier)}`,
        year: yearOf(doc),
      })),
      hasNext: docs.length === 24,
    };
  }

  function metadata(id) {
    const res = net.fetch(`${BASE}/metadata/${encodeURIComponent(id)}`);
    if (res.status !== 200) {
      throw { code: "NOT_FOUND", message: "archive.org item not found", retryable: false };
    }
    return JSON.parse(res.body);
  }

  function getShow(id) {
    const data = metadata(id);
    const meta = data.metadata || {};
    return {
      id: String(id),
      title: text(first(meta.title)) || String(id),
      cover: `${BASE}/services/img/${encodeURIComponent(id)}`,
      synopsis: clean(first(meta.description)),
      year: parseInt(first(meta.year), 10) || undefined,
      genres: ["Animation"],
      status: "finished",
    };
  }

  function getEpisodes(showId) {
    const data = metadata(showId);
    const files = (data.files || [])
      .filter((file) => /\.(mp4|mpeg|ogv|webm)$/i.test(file.name || ""))
      .sort((a, b) => {
        const ah = /h\.264/i.test(a.format || "") ? 0 : 1;
        const bh = /h\.264/i.test(b.format || "") ? 0 : 1;
        return ah - bh;
      });
    return files.map((file, index) => ({
      id: `${showId}|${file.name}`,
      showId: String(showId),
      number: index + 1,
      title:
        String(file.name || "")
          .replace(/\.[^.]+$/, "")
          .replace(/[._]+/g, " ")
          .trim() || `Episode ${index + 1}`,
      durationSec: durationOf(file.length),
    }));
  }

  function resolveEpisode(episodeId) {
    const raw = String(episodeId);
    const split = raw.indexOf("|");
    if (split < 0) {
      throw { code: "PARSE_ERROR", message: "bad episode id", retryable: false };
    }
    const showId = raw.slice(0, split);
    const file = raw.slice(split + 1);
    return [
      {
        url: `${BASE}/download/${encodeURIComponent(showId)}/${encodeURIComponent(file)}`,
        type: "mp4",
        quality: "auto",
      },
    ];
  }

  globalThis.kaminari = { search, getShow, getEpisodes, resolveEpisode };
})();
