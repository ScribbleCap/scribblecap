import { mount } from 'svelte';
import App from './App.svelte';
import Countdown from './overlay/Countdown.svelte';
import Overlay from './overlay/Overlay.svelte';
import './style.css';

// The capture windows load this same bundle with a query (app/src/capture/mod.rs).
const params = new URLSearchParams(location.search);
const target = document.getElementById('app')!;
const overlay = params.get('overlay');
const countdown = params.get('countdown');
if (overlay !== null && Number.isInteger(Number(overlay))) {
  mount(Overlay, { target, props: { display: Number(overlay) } });
} else if (countdown !== null && Number.isInteger(Number(countdown))) {
  mount(Countdown, { target, props: { seconds: Number(countdown) } });
} else {
  mount(App, { target });
}
