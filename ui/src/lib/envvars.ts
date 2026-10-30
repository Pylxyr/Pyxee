/**
 * What Settings > Advanced shows for each variable kept in the `.env` file. The names and their order must match
 * `VARS` in `crates/shell/src/envfile.rs`; the README lists the same variables.
 */
export interface EnvVarInfo {
  name: string;
  title: string;
  /** What it is. */
  about: string;
  /** How to fill it in, or where to get the value. */
  fill: string;
  placeholder: string;
  /** A pick list instead of free text; the empty choice means "not set". */
  choices?: string[];
}

export const ENV_VARS: EnvVarInfo[] = [
  {
    name: 'PYXEE_COOKIES',
    title: 'Cookie file',
    about: "Signs yt-dlp in to your account, so YouTube is less likely to answer \"sign in to confirm you're not a bot\" or \"too many requests\".",
    fill: 'The full path to a cookies.txt file (Netscape format). Sign in to YouTube in your browser, export its cookies with a cookies.txt browser extension, save the file and paste its path here. The file holds your login, so keep it private.',
    placeholder: 'C:\\Users\\you\\cookies.txt',
  },
  {
    name: 'PYXEE_COOKIES_FROM_BROWSER',
    title: 'Cookies from a browser',
    about: 'The same sign-in without a file: yt-dlp reads the cookies straight from a browser on this PC. Ignored when the cookie file above works.',
    fill: 'A browser name, such as firefox. Add a profile after a colon, for example firefox:default-release. Chrome, Edge and Brave often lock their cookies on Windows, so Firefox or a cookie file works more reliably.',
    placeholder: 'firefox',
  },
  {
    name: 'PYXEE_YTDLP',
    title: 'yt-dlp program',
    about: 'Use your own yt-dlp instead of the one Pyxee installs and keeps up to date. While it is set, Pyxee does not update yt-dlp.',
    fill: 'The full path to yt-dlp.exe. Leave it empty to use the built-in one.',
    placeholder: 'C:\\Tools\\yt-dlp.exe',
  },
  {
    name: 'PYXEE_JS_RUNTIME',
    title: 'JavaScript runtime',
    about: 'YouTube needs a JavaScript runtime for yt-dlp. Pyxee tells yt-dlp to use Deno or, if there is none, Node, whichever it finds on your PATH. Set this to choose another one.',
    fill: 'deno, node, quickjs or bun, optionally followed by a colon and the full path (node:C:\\Tools\\node.exe). Type none to pass nothing and let yt-dlp decide. Leave it empty to use what is on your PATH.',
    placeholder: 'node',
  },
  {
    name: 'PYXEE_YTDLP_PLAYER_CLIENT',
    title: 'YouTube player client',
    about: 'The YouTube app yt-dlp pretends to be. Some of them are refused by YouTube\'s servers for a while (the song then fails with HTTP 403). By default Pyxee skips tv_samsung, which is refused as of yt-dlp 2026.10.21.',
    fill: 'A yt-dlp client name such as visionos, or several separated by commas (web_safari,mweb). default,-name drops one from yt-dlp\'s own list. Type none to pass nothing and let yt-dlp decide. Leave it empty to use Pyxee\'s default.',
    placeholder: 'visionos',
  },
  {
    name: 'PYXEE_YTMUSIC_DIRECT',
    title: 'Ask YouTube Music directly',
    about: 'Songs search and the Explore page ask YouTube Music\'s own, undocumented interface that can stop working whenever YouTube changes it. Turn it off to search with yt-dlp only; there is then no Explore page.',
    fill: 'Type 0 to turn it off. Leave it empty (or 1) to keep it on.',
    placeholder: '0',
  },
  {
    name: 'PYXEE_SUGGESTIONS',
    title: 'Search suggestions',
    about: 'While you type in the search box, Pyxee asks Google for completions of what you have typed so far (the same ones YouTube shows). Turn it off and nothing is sent until you run a search.',
    fill: 'Type 0 to turn it off. Leave it empty (or 1) to keep it on.',
    placeholder: '0',
  },
  {
    name: 'PYXEE_LOG',
    title: 'Log detail',
    about: 'How much Pyxee writes to its log file.',
    fill: 'Pick one. Info is the default; choose debug or trace for more detail when you report a problem with a song.',
    placeholder: '',
    choices: ['', 'error', 'warn', 'info', 'debug', 'trace'],
  },
];
