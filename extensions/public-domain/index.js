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

  function search(query, page) {
    const textQuery = query && String(query).trim() ? `title:(${String(query).trim()}) AND ` : "";
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
      synopsis: text(first(meta.description)),
      year: parseInt(first(meta.year), 10) || undefined,
      genres: ["Animation"],
      status: "finished",
    };
  }

  function getEpisodes(showId) {
    const data = metadata(showId);
    const files = (data.files || []).filter((file) =>
      /\.(mp4|mpeg|ogv|webm)$/i.test(file.name || ""),
    );
    return files.map((file, index) => ({
      id: `${showId}|${file.name}`,
      showId: String(showId),
      number: index + 1,
      title: file.name,
      durationSec: parseInt(file.length, 10) || undefined,
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
