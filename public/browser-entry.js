import './app.js';
import init,{mount_browser,render_preview_browser} from './pkg/mdpdf-app.js';
try {
  await init({module_or_path:new URL('./pkg/mdpdf-app.wasm',import.meta.url)});
  window.mdpdfBrowserRender=payload=>JSON.parse(render_preview_browser(payload));
  document.body.replaceChildren();
  mount_browser();
} catch(error) {
  console.error(error);
  document.getElementById('loading')?.replaceChildren(document.createTextNode('The editor could not load. Reload the page to try again.'));
}
