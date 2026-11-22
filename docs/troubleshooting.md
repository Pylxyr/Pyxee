# If something goes wrong

## Songs will not load, or I see "too many requests" or "sign in to confirm you're not a bot"

First try **Settings** -> **Updates** and update.

If it keeps happening, wait a while and try again: YouTube limits how fast one network address can ask for songs, and the limit lifts by itself. Pyxee never signs in to YouTube, and it slows its own requests down on purpose so that it stays under those limits. Changing network (for example, switching off a VPN) can help too.

## A song misbehaves

Pyxee does not write a log unless it is asked to (so a quiet player leaves the disk alone). A build that has logging turned on (see [`development.md`](development.md), `PYXEE_LOG`) writes `pyxee.log`, and **Settings** -> **Diagnostics** -> **Open log folder** opens it; that section is only there while logging is on. If Pyxee ever crashes, it leaves one small file, `last-panic.txt`, in the `logs` folder of its data folder (see [`data-folder.md`](data-folder.md)); attach it to your [issue](https://github.com/Pylxyr/Pyxee/issues). Otherwise, when you open an issue, include what you were playing and what happened.

## A warning about a "JavaScript runtime"

Pyxee still works. To silence the warning and keep every audio format available, install Deno or Node.js, then restart Pyxee. Pyxee finds either on your `PATH`.

| System | Command |
|---|---|
| Windows | `winget install DenoLand.Deno` |
| Arch and CachyOS | `sudo pacman -S deno` |
| Other Linux | Install `nodejs` with your package manager, or get Deno from [deno.com](https://deno.com/) |

Still stuck? [Open an issue](https://github.com/Pylxyr/Pyxee/issues). On Linux, [Pyxee on Linux](linux.md#troubleshooting) has more.

Back to the [README](../README.md).
