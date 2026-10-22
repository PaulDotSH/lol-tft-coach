import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { start } from './lib/store.svelte';
import { forwardLogs } from './lib/log';

forwardLogs();
start();
export default mount(App, { target: document.getElementById('app')! });
