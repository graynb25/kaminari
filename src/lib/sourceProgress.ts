import type { JsEpisode, SourceProgressMark } from "./types";

export function sourceProgressMarks(episodes: JsEpisode[]): SourceProgressMark[] {
  return episodes.map((episode) => ({
    episodeId: episode.id,
    episodeNumber: Math.round(episode.number),
    episodeTitle: episode.title ?? null,
    durationSec: Math.max(0, episode.durationSec ?? 0),
  }));
}

export function anilistProgressThrough(
  mediaEpisodes: number | null | undefined,
  episodes: JsEpisode[],
): number {
  const sourceHighest = episodes.reduce(
    (highest, episode) => Math.max(highest, Math.round(episode.number)),
    0,
  );
  if (sourceHighest <= 0) return 0;
  return mediaEpisodes && mediaEpisodes > 0
    ? Math.min(sourceHighest, mediaEpisodes)
    : sourceHighest;
}
