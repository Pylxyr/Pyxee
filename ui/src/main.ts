import '@fontsource-variable/fredoka/wght.css';
import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import Flyout from './views/Flyout.svelte';
import Mini from './views/Mini.svelte';
import { viewFromSearch } from './lib/windows';

// One bundle, three windows: the main app, the tray flyout and the mini player.
const view = viewFromSearch(location.search);
const Root = view === 'flyout' ? Flyout : view === 'mini' ? Mini : App;
mount(Root, { target: document.getElementById('app')! });
