import { mount } from 'svelte';
import { currentWindow, isTauri } from './lib/tauri.js';
import './app.css';
import App from './App.svelte';

const label = currentWindow().label;
document.documentElement.dataset.win = label;
if (!isTauri) {
  document.documentElement.dataset.demo = '';
  if (new URLSearchParams(location.search).has('bare')) document.documentElement.dataset.bare = '';
}

// Rasa app native: tanpa context menu browser kecuali di kolom teks.
document.addEventListener('contextmenu', (e) => {
  if (!e.target.closest('input, textarea')) e.preventDefault();
});

mount(App, { target: document.getElementById('app'), props: { label } });
