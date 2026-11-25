import { describe, expect, it } from 'vitest';
import { parseYoutubeLink } from './link';

const ID = 'dQw4w9WgXcQ';

describe('parseYoutubeLink', () => {
  it('reads the id of a video from the usual shapes of a link', () => {
    for (const link of [
      `https://www.youtube.com/watch?v=${ID}`,
      `https://youtube.com/watch?v=${ID}&t=42s`,
      `https://music.youtube.com/watch?v=${ID}&si=abc`,
      `https://m.youtube.com/watch?v=${ID}`,
      `https://youtu.be/${ID}?si=abc`,
      `https://www.youtube.com/shorts/${ID}`,
      `https://www.youtube.com/embed/${ID}`,
      `  youtu.be/${ID}  `,
      `www.youtube.com/watch?v=${ID}`,
    ]) {
      expect(parseYoutubeLink(link), link).toEqual({ kind: 'video', id: ID });
    }
  });

  it('a link that names a video and a list is the video; a playlist link is the playlist', () => {
    expect(parseYoutubeLink(`https://www.youtube.com/watch?v=${ID}&list=PLabcdefghij123`)).toEqual({ kind: 'video', id: ID });
    expect(parseYoutubeLink('https://www.youtube.com/playlist?list=PLabcdefghij123')).toEqual({ kind: 'playlist', id: 'PLabcdefghij123' });
    expect(parseYoutubeLink('https://music.youtube.com/playlist?list=OLAK5uy_abc-DEF_123')).toEqual({ kind: 'playlist', id: 'OLAK5uy_abc-DEF_123' });
  });

  it('is not fooled by another host, a login, a port, another scheme or a odd id', () => {
    for (const link of [
      `https://youtube.com.evil.example/watch?v=${ID}`,
      `https://evil.example/youtube.com/watch?v=${ID}`,
      `https://notyoutube.com/watch?v=${ID}`,
      `https://user:pw@www.youtube.com/watch?v=${ID}`,
      `https://www.youtube.com:8443/watch?v=${ID}`,
      `http://www.youtube.com/watch?v=${ID}`,
      `javascript:alert(1)//youtube.com/watch?v=${ID}`,
      `file:///C:/x/youtube.com/watch?v=${ID}`,
      'https://www.youtube.com/watch?v=short',
      'https://www.youtube.com/watch?v=../../etc/passwd',
      `https://www.youtube.com/watch?v=${ID}"; rm -rf`,
      'https://www.youtube.com/playlist?list=a',
      'https://www.youtube.com/playlist?list=PL abc',
      'https://www.youtube.com/',
      'youtube.com',
    ]) {
      expect(parseYoutubeLink(link), link).toBeNull();
    }
  });

  it('leaves ordinary searches and absurd input alone', () => {
    for (const text of ['', '   ', 'lowtide', 'youtube', 'how to watch?v=abc', 'x'.repeat(5000), `https://youtu.be/${ID}\nhttps://youtu.be/${ID}`]) {
      expect(parseYoutubeLink(text)).toBeNull();
    }
  });
});
