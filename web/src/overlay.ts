// The in-game overlay window: its own small page, fed by the `overlay` event (the backend
// decides what and where; this only draws). Outside Tauri it shows a sample over nothing.
import { mount } from 'svelte';
import '@fontsource/barlow-condensed/600.css';
import '@fontsource/barlow-condensed/700.css';
import Overlay from './overlay/Overlay.svelte';
import { startOverlay } from './overlay/store.svelte';

startOverlay();
export default mount(Overlay, { target: document.getElementById('overlay')! });
