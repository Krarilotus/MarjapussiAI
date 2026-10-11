'use strict';

const model = JSON.parse(document.getElementById('model').textContent);
const ns = 'http://www.w3.org/2000/svg';
const $ = id => document.getElementById(id);
const svg = $('graph');
const scroller = $('scroll');
const colors = {
  fact: ['#eaf3fc', '#2464a8'], rule: ['#edf0f3', '#566579'], belief: ['#f4edfb', '#704b9c'],
  future: ['#e8f5f2', '#087569'], decision: ['#fff2e5', '#a25012']
};
const names = {fact: 'Known in the stated context', rule: 'Rule / constraint', belief: 'Belief-based estimate', future: 'Hypothetical continuation', decision: 'Decision'};
let current, nodes, focus, depth, selected, layout, scale = 1, highlight = false;
const history = [];
const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
}[c]));

function element(tag, attributes = {}, text) {
  const result = document.createElementNS(ns, tag);
  for (const [key, value] of Object.entries(attributes)) result.setAttribute(key, value);
  if (text !== undefined) result.textContent = text;
  return result;
}

function lines(text, max = 27) {
  const result = [];
  for (const paragraph of text.split('\n')) {
    let line = '';
    for (const word of paragraph.split(' ')) {
      if (line && line.length + 1 + word.length > max) { result.push(line); line = word; }
      else line += (line ? ' ' : '') + word;
    }
    if (line) result.push(line);
  }
  return result;
}

function remember() { history.push({focus, depth, selected, scale}); }
function maxDepth(id) { return current.max_depths[id]; }
function findLayout() { return current.layouts[`${focus}:${Math.min(depth, maxDepth(focus))}`]; }

function changeView(id) {
  current = model.views.find(view => view.id === id) || model.views[0];
  nodes = new Map(current.nodes.map(node => [node.id, node]));
  focus = current.root;
  depth = Math.min(current.default_depth || 2, maxDepth(focus));
  selected = focus;
  highlight = false;
  history.length = 0;
  render(true);
}

function selectNode(id, center = false) {
  if (!layout.nodes[id]) {
    remember(); focus = id; depth = Math.min(2, maxDepth(id));
    selected = id; highlight = true; render(true); return;
  }
  selected = id; highlight = true; render(false);
  if (center) {
    const node = layout.nodes[id];
    scroller.scrollLeft = Math.max(0, node.x * scale - scroller.clientWidth / 2);
    scroller.scrollTop = Math.max(0, node.y * scale - scroller.clientHeight / 2);
  }
}

function focusSelected() {
  remember(); focus = selected; depth = Math.min(2, maxDepth(focus));
  highlight = false; render(true);
}

function inspect() {
  const node = nodes.get(selected);
  const consumers = current.nodes.filter(n => n.inputs.some(edge => edge.id === selected));
  const links = items => items.map(({id, label}) =>
    '<li><button class="node-link" data-ref="' + esc(id) + '">' + esc(nodes.get(id).label) + '</button>' +
    (label ? '<span class="relation">' + esc(label) + '</span>' : '') + '</li>').join('');
  $('inspector').innerHTML = '<span class="kind">' + esc(names[node.kind]) + ' · ' +
    esc(node.status || 'Proposed definition') + '</span><h2>' + esc(node.label) + '</h2><p>' + esc(node.summary) + '</p>' +
    (node.inputs.length ? '<button id="focus-node">Focus on dependencies</button>' : '') +
    '<p class="epistemic"><strong>Result meaning:</strong> ' + esc(node.epistemic) + '</p>' +
    '<dl><dt>Returns</dt><dd>' + esc(node.returns) + '</dd><dt>Evaluation</dt><dd>' + esc(node.operator) + '</dd></dl>' +
    (node.inputs.length ? '<h3>Requires</h3><ul class="relations">' + links(node.inputs) + '</ul>' : '') +
    (consumers.length ? '<h3>Used by ' + consumers.length + (consumers.length === 1 ? ' concept' : ' concepts') +
      '</h3><ul class="relations">' + links(consumers.map(n => ({id: n.id}))) + '</ul>' : '') +
    (node.detail ? '<details><summary>Conditions and limits</summary><p>' + esc(node.detail) + '</p></details>' : '') +
    (node.bindings ? '<details><summary>Context and sequence</summary><dl>' + Object.entries(node.bindings).map(([key, value]) =>
      '<dt>' + esc(key) + '</dt><dd>' + esc(value) + '</dd>').join('') + '</dl></details>' : '') +
    '<details><summary>Identity and reuse</summary><p>Definition: <code>' + esc(node.id) +
    '</code>. This definition is drawn once. Its runtime queries share a result only when their relevant inputs and bound contexts match.</p></details>';
  $('focus-node')?.addEventListener('click', focusSelected);
  $('inspector').querySelectorAll('[data-ref]').forEach(button => {
    button.addEventListener('click', () => selectNode(button.dataset.ref, true));
  });
}

function render(fit = false) {
  layout = findLayout();
  if (!layout.nodes[selected]) selected = focus;
  svg.replaceChildren();
  svg.setAttribute('viewBox', `0 0 ${layout.width} ${layout.height}`);
  const defs = element('defs');
  const marker = element('marker', {id: 'arrow', viewBox: '0 0 10 10', refX: 9, refY: 5,
    markerWidth: 7, markerHeight: 7, orient: 'auto-start-reverse'});
  marker.append(element('path', {d: 'M 0 0 L 10 5 L 0 10 z', fill: 'context-stroke'}));
  defs.append(marker); svg.append(defs);
  const neighbours = new Set([selected]);
  for (const edge of layout.edges) if (edge.from === selected || edge.to === selected) {
    neighbours.add(edge.from); neighbours.add(edge.to);
  }
  for (const edge of layout.edges) {
    const active = highlight && (edge.from === selected || edge.to === selected);
    const group = element('g', {'data-from': edge.from, 'data-to': edge.to,
      class: 'edge-group' + (active ? ' active' : highlight ? ' muted' : '')});
    group.append(element('path', {d: edge.path, class: 'edge', 'marker-end': 'url(#arrow)'}));
    group.append(element('title', {}, nodes.get(edge.from).label + ' → ' + nodes.get(edge.to).label + ': ' + edge.label));
    svg.append(group);
  }
  for (const [id, geometry] of Object.entries(layout.nodes)) {
    const node = nodes.get(id);
    const [fill, stroke] = colors[node.kind];
    const group = element('g', {'data-node-id': id, role: 'button', tabindex: 0,
      'aria-label': node.label + '. Inspect dependencies and consumers.',
      class: 'node' + (id === selected ? ' selected' : '') + (highlight && !neighbours.has(id) ? ' muted' : ''),
      transform: `translate(${geometry.x + geometry.width / 2},${geometry.y + geometry.height / 2})`, style: `--fill:${fill};--stroke:${stroke}`});
    group.append(element('rect', {x: -geometry.width / 2, y: -geometry.height / 2,
      width: geometry.width, height: geometry.height, rx: node.kind === 'fact' ? 3 : 10, class: 'shape'}));
    const titleLines = lines(node.label);
    const start = -(titleLines.length - 1) * 9 - 5;
    titleLines.forEach((line, index) => group.append(element('text', {x: 0, y: start + index * 18, 'text-anchor': 'middle'}, line)));
    const hidden = node.inputs.filter(input => !layout.nodes[input.id]).length;
    const consumers = current.nodes.filter(n => n.inputs.some(edge => edge.id === id)).length;
    const subtitle = node.badge || (hidden ? `${hidden} deeper ${hidden === 1 ? 'dependency' : 'dependencies'}` :
      consumers > 1 ? `shared by ${consumers} concepts` : names[node.kind].toLowerCase());
    group.append(element('text', {x: 0, y: geometry.height / 2 - 10, 'text-anchor': 'middle', class: 'meta'}, subtitle));
    group.addEventListener('click', () => selectNode(id));
    group.addEventListener('dblclick', () => { selected = id; focusSelected(); });
    group.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); selectNode(id); svg.querySelector(`[data-node-id="${id}"]`)?.focus(); }
      if (event.key === 'ArrowRight' && node.inputs.length) { event.preventDefault(); selected = id; focusSelected(); }
    });
    svg.append(group);
  }
  if (fit) {
    scale = Math.min(1, Math.max(.65, (scroller.clientWidth - 32) / layout.width));
    scroller.scrollTop = 0;
    requestAnimationFrame(() => { scroller.scrollLeft = Math.max(0, layout.nodes[focus].x * scale - scroller.clientWidth / 2); });
  }
  resizeGraph();
  inspect();
  $('scope').textContent = focus === current.root ? current.title : 'Dependencies of: ' + nodes.get(focus).label;
  $('back').disabled = !history.length;
  $('deeper').disabled = depth >= maxDepth(focus);
  $('graph-note').textContent = `${Object.keys(layout.nodes).length} unique definitions · ${layout.edges.length} dependency arrows. ` +
    'Arrows point from an input to the concept that uses it. Select a node to highlight its connections.';
}

function resizeGraph() {
  svg.style.width = layout.width * scale + 'px';
  svg.style.height = layout.height * scale + 'px';
  $('zoom-label').textContent = Math.round(scale * 100) + '%';
}

function back() {
  const previous = history.pop();
  if (!previous) return;
  ({focus, depth, selected, scale} = previous); highlight = false; render(false);
}

$('view').innerHTML = model.views.map(view => '<option value="' + esc(view.id) + '">' + esc(view.title) + '</option>').join('');
$('view').addEventListener('change', event => changeView(event.target.value));
$('reset').onclick = () => changeView(current.id);
$('all').onclick = () => { remember(); focus = current.root; depth = maxDepth(focus); selected = focus; highlight = false; render(true); };
$('deeper').onclick = () => { remember(); depth = Math.min(depth + 1, maxDepth(focus)); highlight = false; render(true); };
$('back').onclick = back;
$('zoom-out').onclick = () => { scale = Math.max(.15, scale / 1.2); resizeGraph(); };
$('zoom-in').onclick = () => { scale = Math.min(2, scale * 1.2); resizeGraph(); };
$('fit').onclick = () => { scale = Math.min(1, (scroller.clientWidth - 32) / layout.width); resizeGraph(); scroller.scrollLeft = 0; };
document.addEventListener('keydown', event => { if (event.key === 'Escape') back(); });

function section() {
  const tab = location.hash === '#example' ? 'example' : location.hash === '#contract' ? 'contract' : location.hash === '#boundary' ? 'boundary' : 'graph';
  document.querySelectorAll('.section').forEach(element => { element.hidden = element.id !== tab + '-section'; });
  document.querySelectorAll('.tabs a').forEach(anchor => {
    if (anchor.dataset.tab === tab) anchor.setAttribute('aria-current', 'page'); else anchor.removeAttribute('aria-current');
  });
  if (tab === 'graph' && layout) resizeGraph();
}
window.addEventListener('hashchange', section);
changeView(model.views[0].id);
section();
