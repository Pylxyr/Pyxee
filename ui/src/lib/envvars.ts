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
    name: 'PYXEE_LOG',
    title: 'Log detail',
    about: 'How much Pyxee writes to its log file.',
    fill: 'Pick one. Info is the default; choose debug or trace for more detail when you report a problem with a song.',
    placeholder: '',
    choices: ['', 'error', 'warn', 'info', 'debug', 'trace'],
  },
  {
    name: 'PYXEE_DISCORD_CLIENT_ID',
    title: 'Discord application ID',
    about: 'Shows "Listening to Pyxee" under your own Discord application name instead of the one built into the release.',
    fill: 'Create an application at discord.com/developers/applications, open it and copy its Application ID (17 to 20 digits). Leave it empty to keep the built-in one.',
    placeholder: '123456789012345678',
  },
];
