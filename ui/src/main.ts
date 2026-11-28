import './app.css';
import { viewFromSearch } from './lib/windows';
import { restoreSettings } from './lib/persist';

// One bundle, three windows: the main app, the tray flyout and the mini player.
const view = viewFromSearch(location.search);

// The page's settings are kept in the data folder as well as in the web view (see lib/persist.ts). Anything the web view
// lost is put back before the app is loaded, because the stores read their settings when their module is first imported;
// that is why the app is imported only after this. Outside the app window this returns at once.
async function start() {
  await restoreSettings({ mirror: view === 'main' });
  const [{ mount }, { default: App }, { default: Flyout }, { default: Mini }] = await Promise.all([
    import('svelte'),
    import('./App.svelte'),
    import('./views/Flyout.svelte'),
    import('./views/Mini.svelte'),
  ]);
  const Root = view === 'flyout' ? Flyout : view === 'mini' ? Mini : App;
  mount(Root, { target: document.getElementById('app')! });
}

void start();
