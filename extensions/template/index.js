// Example Template, the starting point for a Kaminari source.
// Every method is synchronous. The v0.1 host rejects async and await with
// ASYNC_UNSUPPORTED. Replace the paths and field names with your site's own.

(function () {
  const BASE = "https://example.com";
  const PAGE_SIZE = 24; // replace with the page size your site really returns

  // Map an HTTP status onto the error codes the UI shows.
  function codeFor(status) {
    if (status === 404) return "NOT_FOUND";
    if (status === 429) return "RATE_LIMITED";
    if (status === 401 || status === 403) return "AUTH_REQUIRED";
    return "PARSE_ERROR";
  }

  // Fetch a path from the allowlisted host and parse the JSON body.
  function getJson(path) {
    const res = net.fetch(BASE + path);
    if (res.status !== 200) {
      throw {
        code: codeFor(res.status),
        message: "example.com answered " + res.status,
        retryable: res.status === 429 || res.status >= 500,
      };
    }
    return JSON.parse(res.body);
  }

  // Years arrive as 2026 or as "2026". Anything unparseable is unknown.
  function yearOf(value) {
    const year = parseInt(String(value), 10);
    return Number.isFinite(year) ? year : undefined;
  }

  // search(query, page) lists matching shows.
  // query is the text the user typed. page starts at 1.
  function search(query, page) {
    const path = "/api/search?q=" + encodeURIComponent(String(query)) + "&page=" + (page || 1);
    const data = getJson(path);

    // Placeholder: point at the array your own API returns.
    const rows = Array.isArray(data.results) ? data.results : [];
    return {
      items: rows.map((row) => ({
        // Ids stay strings end to end. Registry ids exceed 2 to the 53.
        id: String(row.id),
        title: String(row.title),
        cover: row.poster ? String(row.poster) : undefined,
        year: yearOf(row.year),
      })),
      hasNext: rows.length >= PAGE_SIZE,
    };

    // Site serves HTML instead of JSON? Drop the JSON path above and use this:
    // const doc = net.fetch(BASE + "/search?q=" + encodeURIComponent(String(query))).body;
    // const cards = html.select(doc, ".card");
    // return {
    //   items: cards.map((card) => ({
    //     id: String(card.attrs["data-id"]),
    //     title: card.text.trim(),
    //   })),
    //   hasNext: cards.length >= PAGE_SIZE,
    // };
  }

  // getShow(id) returns one show with its detail fields.
  function getShow(id) {
    const data = getJson("/api/show/" + encodeURIComponent(String(id)));
    return {
      id: String(id),
      title: String(data.title || id),
      cover: data.poster ? String(data.poster) : undefined,
      synopsis: data.summary ? String(data.summary) : undefined,
      year: yearOf(data.year),
      genres: Array.isArray(data.genres) ? data.genres.map(String) : undefined,
      status: "unknown",
    };
  }

  // getEpisodes(showId) returns the episode list for a show.
  function getEpisodes(showId) {
    const data = getJson("/api/show/" + encodeURIComponent(String(showId)) + "/episodes");
    // Placeholder: point at the array your own API returns.
    const rows = Array.isArray(data.episodes) ? data.episodes : [];
    return rows.map((row, index) => ({
      id: String(row.id),
      showId: String(showId),
      number: Number.isFinite(row.number) ? row.number : index + 1,
      title: String(row.title || "Episode " + (index + 1)),
    }));
  }

  // resolveEpisode(episodeId) returns playable streams.
  // episodeId is the id getEpisodes produced, so keep it opaque and as a string.
  function resolveEpisode(episodeId) {
    const data = getJson("/api/stream/" + encodeURIComponent(String(episodeId)));
    // One entry plays. Add more when the site offers qualities.
    return [
      {
        url: String(data.url),
        type: "mp4", // one of "hls", "mp4", "dash", "embed"
        quality: "auto",
        // Headers are optional. Send them only when the site checks the request,
        // and list every host they point at in permissions.hosts.
        // headers: { Referer: "https://example.com/", "User-Agent": "Mozilla/5.0" },
        // Subtitles ride along when the site ships tracks.
        // subtitles: [{ lang: "en", url: "https://example.com/subs/en.vtt" }],
      },
    ];
  }

  globalThis.kaminari = { search, getShow, getEpisodes, resolveEpisode };
})();
