import { EditorView, basicSetup, css, oneDark } from './vendor/editor.js';
const labels = {
  page:'Page', typography:'Typography', headings:'Headings', tables:'Tables', code:'Code blocks', diagrams:'Diagrams', images:'Images', header_footer:'Header & footer', custom_css:'Custom CSS',
  size:'Paper size', orientation:'Orientation', margins:'Margins · mm', background:'Background', toc:'Table of contents', toc_depth:'TOC depth', heading_numbers:'Number headings', break_before_h1:'New page before H1',
  body_font:'Body font',heading_font:'Heading font',mono_font:'Monospace font',base_size:'Body size · pt',line_height:'Line height',paragraph_spacing:'Paragraph spacing · em',text_color:'Text color',link_color:'Link color',text_align:'Text alignment',hyphenation:'Hyphenation',
  border_color:'Border color',border_width:'Border width · px',header_bg:'Header background',header_color:'Header text',header_weight:'Header weight',zebra:'Alternating row colors',zebra_color:'Alternating row color',cell_padding_vertical:'Vertical padding · px',cell_padding_horizontal:'Horizontal padding · px',font_size:'Font size',width:'Width',repeat_header:'Repeat table headers',avoid_row_split:'Keep rows together',wide_tables:'Wide tables',caption_position:'Caption position',
  custom_syntax_css:'Imported syntax styles',syntax_theme:'Syntax theme',border:'Border',line_numbers:'Line numbers',wrap:'Wrap long lines',avoid_break_under_lines:'Keep blocks under · lines',mermaid_theme:'Diagram theme',theme_variables:'Theme colors',font_family:'Diagram font',max_width:'Maximum width · %',align:'Alignment',oversize:'Oversized diagrams',captions:'Figure captions',numbering:'Figure numbering',flowchart_curve:'Flowchart curve',sequence_mirror_actors:'Mirror sequence actors',
  border_radius:'Corner radius · px',shadow:'Image shadow',caption_size:'Caption size · em',caption_color:'Caption color',font_size:'Font size',color:'Color',hide_on_first_page:'Hide on first page',size:'Size',weight:'Font weight',margin_top:'Top spacing · em',margin_bottom:'Bottom spacing · em',bottom_border:'Bottom border',uppercase:'Uppercase', name:'Theme name',
};
const choices = {
  'page.orientation':['portrait','landscape'],'typography.text_align':['left','justify'],'tables.style':['horizontal-lines','grid','minimal','none'],'tables.width':['full','auto'],'tables.wide_tables':['shrink-to-fit','wrap','rotate-landscape'],'tables.caption_position':['above','below'],
  'code.syntax_theme':['github-light','one-dark','solarized-light','solarized-dark','nord'],'diagrams.mermaid_theme':['neutral','default','dark','forest','base'],'diagrams.align':['left','center','right'],'images.align':['left','center','right'],'diagrams.oversize':['fit-page','allow-split','none'],'diagrams.flowchart_curve':['basis','linear','step','cardinal'],
};
const builtFonts = ['Inter','Source Serif 4','Merriweather','IBM Plex Sans','JetBrains Mono','Noto Sans','Noto Sans CJK SC'];
const captions = {a3:'A3',a4:'A4',a5:'A5',letter:'US Letter',legal:'US Legal',custom:'Custom size',portrait:'Portrait',landscape:'Landscape','horizontal-lines':'Horizontal lines','shrink-to-fit':'Shrink to fit','rotate-landscape':'Landscape page','fit-page':'Fit on one page','allow-split':'Allow page splits','github-light':'GitHub Light','one-dark':'One Dark','solarized-light':'Solarized Light','solarized-dark':'Solarized Dark',nord:'Nord'};
const title = text => captions[text] || text.replaceAll('_',' ').replace(/\b\w/g, c => c.toUpperCase());
function el(tag, className, text) { const node = document.createElement(tag); if (className) node.className = className; if (text !== undefined) node.textContent = text; return node; }
export function settings({ theme, preset, fonts, onChange, onReset }) {
  const container = document.getElementById('settings-controls');
  const open = new Set([...container.querySelectorAll('details[open]')].map(node => node.dataset.section));
  for (const editor of container._editors || []) editor.destroy();
  container._editors = [];
  container.replaceChildren();
  function field(path, value, parent) {
    const key = path.split('.').at(-1);
    const labelText = path === 'page.size' ? 'Paper size' : labels[key] || title(key);
    const id = 'theme-' + path.replaceAll('.','-');
    const wrapper = el('div', 'field');
    if (typeof value === 'boolean') {
      const row = el('label','switch-row'); row.htmlFor = id;
      row.append(el('span','',labelText));
      const input = el('input'); input.type = 'checkbox'; input.id = id; input.dataset.themePath = path; input.setAttribute('role','switch'); input.checked = value;
      input.addEventListener('change', () => onChange(path,input.checked)); row.append(input); parent.append(row); return;
    }
    const label = el('label','',labelText); label.htmlFor = id; wrapper.append(label);
    let input;
    const fontField = /(?:body_font|heading_font|mono_font|font_family)$/.test(path);
    const variants = choices[path] || (path === 'page.size' ? ['a3','a4','a5','letter','legal','custom'] : null);
    if (variants || fontField) {
      input = el('select');
      let options = variants;
      if (fontField) options = [...(path === 'diagrams.font_family' ? [''] : []), ...builtFonts, ...fonts.map(f => 'assetfont-' + f.id)];
      if(fontField && value && !options.includes(value))options=[...options,value];
      for (const option of options) {
        const name = fontField ? fonts.find(f => 'assetfont-' + f.id === option)?.name || (option || 'Follow body font') : title(option);
        const node = el('option','',name); node.value = option; if (fontField && option) node.style.fontFamily = option; input.append(node);
      }
      input.value = path === 'page.size' && typeof value === 'object' ? 'custom' : value ?? '';
      input.addEventListener('change', () => {
        const next = path === 'page.size' && input.value === 'custom' ? { custom: { width:210,height:297 } } : fontField && !input.value ? null : input.value;
        const valid = onChange(path,next);
        if (valid !== false && (path === 'page.size' || path === 'diagrams.mermaid_theme')) settings({ theme, preset, fonts, onChange, onReset });
        if (fontField && sample) sample.style.fontFamily = input.value || theme.typography.body_font;
      });
    } else if (typeof value === 'number') {
      input = el('input'); input.type = 'number'; input.value = value;
      input.step = /weight$/.test(path) ? '100' : /toc_depth|avoid_break_under_lines/.test(path) ? '1' : 'any';
      const exactRanges={'tables.font_size':[.7,1],'code.font_size':[6,24],'diagrams.font_size':[6,72],'header_footer.font_size':[6,16],'tables.border_width':[0,3],'tables.header_weight':[400,800],'images.caption_size':[.5,2],'images.border_radius':[0,100],'typography.paragraph_spacing':[0,5]};
      const range = exactRanges[path] || (/page.size.custom/.test(path) ? [50,2000] : /weight$/.test(path) ? [100,900] : /toc_depth/.test(path) ? [1,4] : /base_size/.test(path) ? [8,16] : /line_height/.test(path) ? [1,2] : /max_width/.test(path) ? [1,100] : /margins/.test(path) ? [0,100] : /cell_padding/.test(path) ? [0,50] : /\.size$/.test(path) ? [6,96] : /heading.*margin/.test(path) ? [0,10] : [0,1000]);
      input.min = range[0]; input.max = range[1];
      input.addEventListener('input', () => {
        if (input.value === '' || !input.checkValidity()) return;
        input.setAttribute('aria-invalid', String(onChange(path,Number(input.value)) === false));
      });
    } else if (path === 'code.custom_syntax_css') {
      input = el('textarea','json-field'); input.rows = 3; input.value = value || ''; input.placeholder = 'Use a bundled theme or import a .tmTheme file'; input.addEventListener('change',()=>onChange(path,input.value || null));
    } else if (path === 'diagrams.theme_variables') {
      input = el('textarea','json-field'); input.rows = 4; input.value = JSON.stringify(value || {},null,2);
      input.addEventListener('change', () => {
        try { const next = JSON.parse(input.value); if (!next || Array.isArray(next) || typeof next !== 'object' || Object.values(next).some(v => typeof v !== 'string')) throw new Error(); onChange(path,next); input.removeAttribute('aria-invalid'); }
        catch { input.setAttribute('aria-invalid','true'); }
      });
    } else {
      input = el('input'); input.type = 'text'; input.value = value ?? '';
      if (value === null) input.placeholder = 'None';
      input.addEventListener('input', () => { input.setAttribute('aria-invalid', String(onChange(path,input.value || (value === null ? null : '')) === false)); });
    }
    input.id = id; input.dataset.themePath = path;
    const colorField = /(?:color|_bg|background)$/i.test(path) && typeof value === 'string';
    if (colorField) {
      const group = el('div','color-field'); const picker = el('input'); picker.type = 'color'; picker.setAttribute('aria-label', labelText + ' picker'); picker.value = /^#[a-f\d]{6}$/i.test(value) ? value : '#ffffff';
      picker.addEventListener('input', () => { input.value = picker.value; onChange(path,picker.value); });
      group.append(picker,input); wrapper.append(group);
    } else wrapper.append(input);
    let sample;
    if (fontField) {
      sample = el('p','font-sample','The quick brown fox.'); sample.style.fontFamily = value || theme.typography.body_font; wrapper.append(sample);
      const system=el('input');system.type='text';system.placeholder='Or enter a system font';system.setAttribute('aria-label',labelText+' system font');
      system.addEventListener('change',()=>{const name=system.value.trim();if(!name)return;if(onChange(path,name)!==false){if(![...input.options].some(option=>option.value===name))input.add(new Option(name,name));input.value=name;sample.style.fontFamily=name;}});wrapper.append(system);
      wrapper.append(el('p','field-help','System fonts must also be installed on the PDF server.'));
    }
    parent.append(wrapper);
    if (path === 'page.size' && typeof value === 'object') {
      const pair = el('div','field-pair'); field('page.size.custom.width',value.custom.width,pair); field('page.size.custom.height',value.custom.height,pair); parent.append(pair);
    }
  }
  const sections = ['page','typography','headings','tables','code','diagrams','images','header_footer','custom_css'];
  for (const section of sections) {
    const details = el('details','settings-section'); details.dataset.section = section;
    details.open = open.has(section) || (open.size === 0 && section === 'page');
    const summary = el('summary','',labels[section]); const body = el('div','settings-section-body'); details.append(summary,body);
    if (section === 'custom_css') {
      const label = el('label','field-help','Print-safe CSS. External URLs and imports are blocked.'); body.append(label);
      const host = el('div','css-editor'); host.dataset.themePath = section; body.append(host);
      const editor = new EditorView({ doc:theme.custom_css || '', parent:host, extensions:[basicSetup,css(),oneDark,EditorView.lineWrapping,EditorView.contentAttributes.of({'aria-label':'Custom CSS'}), EditorView.updateListener.of(update => { if (update.docChanged) onChange('custom_css',update.state.doc.toString() || null); })] });
      container._editors.push(editor);
    } else {
      const data = theme[section];
      if (section === 'code') {
        const upload = el('button','section-reset','Import .tmTheme'); upload.type='button';
        const file = el('input'); file.type='file'; file.accept='.tmTheme'; file.hidden=true;
        upload.onclick=()=>file.click();
        file.onchange=async()=>{try{const selected=file.files[0];if(!selected)return;if(selected.size>2*1024*1024)throw new Error('Syntax theme exceeds 2 MB');const response=await fetch('/api/syntax/import',{method:'POST',headers:{'Content-Type':'application/octet-stream'},body:await selected.arrayBuffer()});const result=await response.json();if(!response.ok)throw new Error(result.message);onChange('code.custom_syntax_css',result.css);settings({theme,preset,fonts,onChange,onReset});}catch(error){const note=el('p','field-help',error.message);note.setAttribute('role','alert');body.append(note);}finally{file.value='';}};
        body.append(upload,file);
      }
      if (section === 'page') {
        field('page.size',data.size,body); field('page.orientation',data.orientation,body);
        const heading = el('p','field-heading','Margins · mm'); body.append(heading);
        const grid = el('div','margin-grid'); for (const [key,value] of Object.entries(data.margins)) field('page.margins.'+key,value,grid); body.append(grid);
        for (const key of ['background','toc','toc_depth','heading_numbers','break_before_h1']) field('page.'+key,data[key],body);
      } else if (section === 'headings') {
        data.levels.forEach((level,index) => { body.append(el('div','nested-heading','Heading ' + (index + 1))); for (const [key,value] of Object.entries(level)) field(`headings.levels.${index}.${key}`,value,body); });
      } else if (section === 'header_footer') {
        for (const slot of ['header','footer']) { body.append(el('div','field-heading',title(slot))); for (const [key,value] of Object.entries(data[slot])) field(`header_footer.${slot}.${key}`,value,body); }
        const help = el('p','field-help','Tokens: {title}, {author}, {date}, {page}, {pages}, {section}'); body.append(help);
        for (const key of ['font_size','color','hide_on_first_page']) field('header_footer.'+key,data[key],body);
      } else {
        for (const [key,value] of Object.entries(data)) {
          if (section === 'diagrams' && key === 'theme_variables') {
            if (data.mermaid_theme === 'base') {
              body.append(el('p','field-heading','Base theme colors'));
              for (const name of ['primaryColor','secondaryColor','tertiaryColor','lineColor','textColor','noteBkgColor']) field('diagrams.theme_variables.'+name,data.theme_variables[name] || '#e8ece5',body);
            }
          }
          field(section+'.'+key,value,body);
        }
      }
    }
    const reset = el('button','section-reset','↺ Reset to preset'); reset.type = 'button'; reset.addEventListener('click', () => onReset(section)); body.append(reset);
    container.append(details);
  }
  const themeName = el('div','settings-intro'); field('name',theme.name,themeName); container.append(themeName);
}
