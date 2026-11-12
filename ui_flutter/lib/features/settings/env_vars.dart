/// What Settings > Advanced shows for each variable kept in the `.env` file. The names have to be real settings
/// (`VARS` in `crates/shell/src/envfile.rs`) in the same order; a Rust test checks it. `PYXEE_PERF` and
/// `PYXEE_SEEK_MODE` belonged to the old window and are not listed.
class EnvVarInfo {
  const EnvVarInfo({
    required this.name,
    required this.title,
    required this.about,
    required this.fill,
    required this.placeholder,
    this.choices,
  });

  final String name;
  final String title;

  /// What it is.
  final String about;

  /// How to fill it in, or where to get the value.
  final String fill;
  final String placeholder;

  /// A pick list instead of free text; the empty choice means "not set".
  final List<String>? choices;
}

const envVars = <EnvVarInfo>[
  EnvVarInfo(
    name: 'PYXEE_COOKIES',
    title: 'Cookie file',
    about: "Signs yt-dlp in to your account, so YouTube is less likely to answer \"sign in to confirm you're not a bot\" or \"too many requests\".",
    fill: 'The full path to a cookies.txt file (Netscape format). Sign in to YouTube in your browser, export its cookies with a cookies.txt browser extension, save the file and paste its path here. The file holds your login, so keep it private.',
    placeholder: r'C:\Users\you\cookies.txt',
  ),
  EnvVarInfo(
    name: 'PYXEE_COOKIES_FROM_BROWSER',
    title: 'Cookies from a browser',
    about: 'The same sign-in without a file: yt-dlp reads the cookies straight from a browser on this PC. Ignored when the cookie file above works.',
    fill: 'A browser name, such as firefox. Add a profile after a colon, for example firefox:default-release. Chrome, Edge and Brave often lock their cookies on Windows, so Firefox or a cookie file works more reliably.',
    placeholder: 'firefox',
  ),
  EnvVarInfo(
    name: 'PYXEE_YTDLP',
    title: 'yt-dlp program',
    about: 'Use your own yt-dlp instead of the one Pyxee installs and keeps up to date. While it is set, Pyxee does not update yt-dlp.',
    fill: 'The full path to yt-dlp.exe. Leave it empty to use the built-in one.',
    placeholder: r'C:\Tools\yt-dlp.exe',
  ),
  EnvVarInfo(
    name: 'PYXEE_JS_RUNTIME',
    title: 'JavaScript runtime',
    about: 'YouTube needs a JavaScript runtime for yt-dlp. Pyxee tells yt-dlp to use Deno or, if there is none, Node, whichever it finds on your PATH. Set this to choose another one.',
    fill: r'deno, node, quickjs or bun, optionally followed by a colon and the full path (node:C:\Tools\node.exe). Type none to pass nothing and let yt-dlp decide. Leave it empty to use what is on your PATH.',
    placeholder: 'node',
  ),
  EnvVarInfo(
    name: 'PYXEE_YTDLP_PLAYER_CLIENT',
    title: 'YouTube player client',
    about: "The YouTube app yt-dlp pretends to be. Some of them are refused by YouTube's servers for a while (the song then fails with HTTP 403). By default Pyxee skips tv_samsung, which is refused as of yt-dlp 2026.10.21.",
    fill: "A yt-dlp client name such as visionos, or several separated by commas (web_safari,mweb). default,-name drops one from yt-dlp's own list. Type none to pass nothing and let yt-dlp decide. Leave it empty to use Pyxee's default.",
    placeholder: 'visionos',
  ),
  EnvVarInfo(
    name: 'PYXEE_YTMUSIC_DIRECT',
    title: 'Ask YouTube Music directly',
    about: "Songs search and the Explore page ask YouTube Music's own, undocumented interface that can stop working whenever YouTube changes it. Turn it off to search with yt-dlp only; there is then no Explore page.",
    fill: 'Type 0 to turn it off. Leave it empty (or 1) to keep it on.',
    placeholder: '0',
  ),
  EnvVarInfo(
    name: 'PYXEE_SUGGESTIONS',
    title: 'Search suggestions',
    about: 'While you type in the search box, Pyxee asks Google for completions of what you have typed so far (the same ones YouTube shows). Turn it off and nothing is sent until you run a search.',
    fill: 'Type 0 to turn it off. Leave it empty (or 1) to keep it on.',
    placeholder: '0',
  ),
  EnvVarInfo(
    name: 'PYXEE_LOG',
    title: 'Log detail',
    about: 'How much Pyxee prints to the console while it runs. There is no log file in this version.',
    fill: 'Pick one. Warn is the default, so a quiet app prints next to nothing; choose info, debug or trace for more detail when you report a problem with a song. Start Pyxee from a terminal to see it. Used from the next start.',
    placeholder: '',
    choices: ['', 'error', 'warn', 'info', 'debug', 'trace'],
  ),
];
