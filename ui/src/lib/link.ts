/** What a pasted `YouTube` link points to: one video, or a playlist. Nothing but the id is ever taken from it. */
export type YoutubeLink = { kind: 'video'; id: string } | { kind: 'playlist'; id: string };

/** The only hosts a link may name (exactly, so `youtube.com.example.org` and the like are no link). */
const HOSTS = new Set(['youtube.com', 'www.youtube.com', 'm.youtube.com', 'music.youtube.com', 'youtu.be']);
const VIDEO_ID = /^[A-Za-z0-9_-]{11}$/;
const LIST_ID = /^[A-Za-z0-9_-]{10,64}$/;
/** A link longer than this is not one that was pasted from an address bar. */
const MAX_LENGTH = 300;

/**
 * Reads a pasted `YouTube` or `YouTube Music` link: https only (or the address as it is shown without the scheme), no
 * login, no port, one of the hosts above, and an id made of plain characters. Anything else is `null`, so the text is
 * searched for like any other. The link itself is never sent anywhere: only the id that was read from it.
 */
export function parseYoutubeLink(text: string): YoutubeLink | null {
  const raw = text.trim();
  if (raw.length === 0 || raw.length > MAX_LENGTH || /\s/.test(raw)) return null;
  let url: URL;
  try {
    url = new URL(raw.includes('://') ? raw : `https://${raw}`);
  } catch {
    return null;
  }
  if (url.protocol !== 'https:' || url.username || url.password || url.port) return null;
  const host = url.hostname.toLowerCase();
  if (!HOSTS.has(host)) return null;
  const parts = url.pathname.split('/').filter(Boolean);
  let video: string | null = null;
  if (host === 'youtu.be') video = parts[0] ?? null;
  else if (parts[0] === 'watch') video = url.searchParams.get('v');
  else if ((parts[0] === 'shorts' || parts[0] === 'embed' || parts[0] === 'live') && parts[1]) video = parts[1];
  // A link that names a video and a list is the video: the list of such a link is often an automatic mix.
  if (video !== null && VIDEO_ID.test(video)) return { kind: 'video', id: video };
  if (parts[0] === 'playlist') {
    const list = url.searchParams.get('list');
    if (list !== null && LIST_ID.test(list)) return { kind: 'playlist', id: list };
  }
  return null;
}
