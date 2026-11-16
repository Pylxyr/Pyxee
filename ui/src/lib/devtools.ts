/**
 * Developer tools: the browser preview against the mock backend, the performance log and the seek-bar fill modes.
 * `npm run dev` / `tauri dev` have them; a production build (what a release is made from) has none of them, and the
 * bundler drops the mock backend from it. A build that needs them (the measuring script) sets `VITE_PYXEE_DEV_TOOLS=1`;
 * the single-file preview (`SINGLE=1`) has them too. (`__DEV_TOOLS__` is a literal from vite.config.ts.)
 */
export const DEV_TOOLS: boolean = __DEV_TOOLS__;
