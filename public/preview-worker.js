import init, { render_preview_local } from './pkg/mdpdf-app.js';

// cargo-leptos publishes the binary without wasm-bindgen's `_bg` suffix.
const ready = init({module_or_path: new URL('./pkg/mdpdf-app.wasm', self.location.href)});
void ready.catch(() => {}); // Report initialization failures with the request ID.
const yieldToMessages = () => new Promise(resolve => setTimeout(resolve,0));
let pending, running = false;
self.onmessage = event => {
  // While WASM initializes, keep only the newest document.
  pending = event.data;
  if (running) return;
  running = true;
  void (async () => {
    try {
      await ready;
      await yieldToMessages();
      while (pending) {
        const { id, payload } = pending;
        pending = null;
        try { self.postMessage({ id, result: JSON.parse(render_preview_local(payload)) }); }
        catch (error) { self.postMessage({ id, error: String(error) }); }
        await yieldToMessages();
      }
    } catch (error) {
      if (pending) self.postMessage({ id: pending.id, error: String(error) });
      pending = null;
    } finally { running = false; }
  })();
};
