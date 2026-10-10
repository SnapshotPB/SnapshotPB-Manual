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
    document.body.classList.toggle('nav-open', open);
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

// The tab groups. A section has two tabs, the board and the configuration app, and the app
// tab has one tab for each platform where the platforms differ. A choice on one group selects
// the same tab on every group of the same kind, and the page remembers each choice, as a
// reference that shows one platform at a time does.
(function () {
  'use strict';
  var KEY = 'snapshotpb-manual-';
  var GROUPS = ['view', 'target'];
  function select(target) {
    var inputs = document.querySelectorAll('.tabs > input.t-' + target);
    for (var i = 0; i < inputs.length; i++) { inputs[i].checked = true; }
  }
  function groupOf(el) {
    var tabs = el.parentNode;
    return (tabs && tabs.getAttribute && tabs.getAttribute('data-group')) || GROUPS[0];
  }
  GROUPS.forEach(function (group) {
    var stored = null;
    try { stored = localStorage.getItem(KEY + group); } catch (e) { stored = null; }
    if (stored && document.querySelector('.tabs[data-group="' + group + '"] > input.t-' + stored)) { select(stored); }
  });
  document.addEventListener('change', function (ev) {
    var el = ev.target;
    if (!el || !el.classList || !el.classList.contains('tab-input')) { return; }
    var target = (el.className.match(/\bt-([a-z0-9_]+)/) || [])[1];
    if (!target) { return; }
    select(target);
    try { localStorage.setItem(KEY + groupOf(el), target); } catch (e) { /* no storage: the choice lasts for this page only */ }
  });
})();
