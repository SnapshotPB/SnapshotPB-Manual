// The page works with no script. The script adds the theme switch, the contents drawer and
// the marker of the section that is on the screen.
(function () {
  'use strict';
  var root = document.documentElement;
  var KEY = 'snapshotpb-manual-theme';
  var order = ['auto', 'light', 'dark'];
  var titles = { auto: 'Theme: follows the system', light: 'Theme: light', dark: 'Theme: dark' };

  function stored() {
    try { return localStorage.getItem(KEY) || 'auto'; } catch (e) { return 'auto'; }
  }
  function apply(mode) {
    if (mode === 'auto') { root.removeAttribute('data-theme'); } else { root.setAttribute('data-theme', mode); }
    var btn = document.getElementById('themebtn');
    if (btn) { btn.title = titles[mode]; btn.setAttribute('aria-label', titles[mode] + '. Press to change it'); }
  }
  apply(stored());
  var themebtn = document.getElementById('themebtn');
  if (themebtn) {
    themebtn.addEventListener('click', function () {
      var next = order[(order.indexOf(stored()) + 1) % order.length];
      try { localStorage.setItem(KEY, next); } catch (e) { /* no storage: the choice lasts for this page only */ }
      apply(next);
    });
  }

  // The drawer on a narrow screen.
  var nav = document.getElementById('nav');
  var navbtn = document.getElementById('navbtn');
  var scrim = document.getElementById('scrim');
  function openNav(open) {
    if (!nav) { return; }
    nav.classList.toggle('open', open);
    if (scrim) { scrim.hidden = !open; }
    if (navbtn) { navbtn.setAttribute('aria-expanded', open ? 'true' : 'false'); }
  }
  if (navbtn) { navbtn.addEventListener('click', function () { openNav(!nav.classList.contains('open')); }); }
  if (scrim) { scrim.addEventListener('click', function () { openNav(false); }); }
  if (nav) {
    nav.addEventListener('click', function (ev) {
      if (ev.target && ev.target.tagName === 'A') { openNav(false); }
    });
  }
  document.addEventListener('keydown', function (ev) { if (ev.key === 'Escape') { openNav(false); } });

  // The entry of the contents that matches the section on the screen.
  var links = nav ? Array.prototype.slice.call(nav.querySelectorAll('a[href^="#"]')) : [];
  var byId = {};
  links.forEach(function (a) { byId[a.getAttribute('href').slice(1)] = a; });
  var heads = Array.prototype.slice.call(document.querySelectorAll('article h2[id], article h3[id]'))
    .filter(function (h) { return byId[h.id]; });
  var current = null;
  function mark(id) {
    if (id === current) { return; }
    current = id;
    links.forEach(function (a) { a.classList.toggle('active', a.getAttribute('href') === '#' + id); });
    var a = byId[id];
    if (a && nav && !nav.classList.contains('open')) {
      var top = a.offsetTop - nav.clientHeight / 2;
      if (Math.abs(nav.scrollTop - top) > nav.clientHeight / 2) { nav.scrollTop = top; }
    }
  }
  function update() {
    var line = 90;
    var best = null;
    for (var i = 0; i < heads.length; i++) {
      if (heads[i].getBoundingClientRect().top <= line) { best = heads[i]; } else { break; }
    }
    if (!best && heads.length) { best = heads[0]; }
    if (best) { mark(best.id); }
  }
  var ticking = false;
  window.addEventListener('scroll', function () {
    if (!ticking) { ticking = true; requestAnimationFrame(function () { ticking = false; update(); }); }
  }, { passive: true });
  window.addEventListener('resize', update);
  update();
})();

// The runtime selector: the reader selects the runtimes that matter, and each block shows
// while one of its runtimes is selected. A section with no block left folds to one line.
// The page remembers the selection.
(function () {
  'use strict';
  var KEY = 'snapshotpb-manual-runtimes';
  var bar = document.getElementById('runtime-bar');
  if (!bar) { return; }
  var inputs = Array.prototype.slice.call(bar.querySelectorAll('input[data-runtime]'));
  var all = inputs.map(function (i) { return i.getAttribute('data-runtime'); });
  var blocks = Array.prototype.slice.call(document.querySelectorAll('[data-runtimes]'));
  var sections = Array.prototype.slice.call(document.querySelectorAll('[data-section]'));
  sections.forEach(function (s) {
    var note = document.createElement('p');
    note.className = 'folded-note';
    note.textContent = 'Nothing for the runtimes you selected. Select another runtime above.';
    var head = s.querySelector('.crumbs') || s.querySelector('h3, h4');
    if (head && head.nextSibling) { s.insertBefore(note, head.nextSibling); } else { s.appendChild(note); }
  });
  function selected() {
    var out = {};
    inputs.forEach(function (i) { if (i.checked) { out[i.getAttribute('data-runtime')] = true; } });
    return out;
  }
  function apply() {
    var on = selected();
    blocks.forEach(function (b) {
      var keys = b.getAttribute('data-runtimes').split(/\s+/);
      var show = keys.some(function (k) { return on[k]; });
      b.classList.toggle('hidden', !show);
    });
    sections.forEach(function (s) {
      var any = Array.prototype.some.call(s.querySelectorAll('.variant'), function (v) { return !v.classList.contains('hidden'); });
      s.classList.toggle('folded', !any);
    });
    try { localStorage.setItem(KEY, Object.keys(on).join(' ')); } catch (e) { /* no storage */ }
  }
  var stored = null;
  try { stored = localStorage.getItem(KEY); } catch (e) { stored = null; }
  if (stored !== null) {
    var set = {};
    stored.split(/\s+/).forEach(function (k) { set[k] = true; });
    var known = all.some(function (k) { return set[k]; });
    if (known) { inputs.forEach(function (i) { i.checked = !!set[i.getAttribute('data-runtime')]; }); }
  }
  bar.addEventListener('change', apply);
  var allBtn = document.getElementById('runtime-all');
  if (allBtn) { allBtn.addEventListener('click', function () { inputs.forEach(function (i) { i.checked = true; }); apply(); }); }
  apply();
})();
