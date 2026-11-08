// Example AnimeProvider export. Real calls go through Rust first.
// This file is the shape extension authors copy.

export async function search(query) {
  throw new Error("NOT_IMPLEMENTED");
}
export async function getShow(id) {
  throw new Error("NOT_IMPLEMENTED");
}
export async function getEpisodes(showId) {
  throw new Error("NOT_IMPLEMENTED");
}
export async function resolveEpisode(ep) {
  throw new Error("NOT_IMPLEMENTED");
}
