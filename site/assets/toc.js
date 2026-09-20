// Marks the section being read in the sidebar. The heading nearest above a
// line a little below the viewport top wins, which is what tracks the eye
// while scrolling; the last heading wins once the page bottoms out, so the
// final section is reachable even when it is shorter than the viewport.
(function () {
  var toc = document.querySelector('.toc');
  if (!toc) return;

  var items = [];
  Array.prototype.forEach.call(toc.querySelectorAll('a[href^="#"]'), function (a) {
    var el = document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1)));
    if (el) items.push({ el: el, a: a });
  });
  if (!items.length) return;

  var active = null;

  function update() {
    var line = window.pageYOffset + 120;
    var current = items[0];
    for (var i = 0; i < items.length; i++) {
      if (items[i].el.getBoundingClientRect().top + window.pageYOffset <= line) current = items[i];
    }
    if (window.innerHeight + window.pageYOffset >= document.documentElement.scrollHeight - 2) {
      current = items[items.length - 1];
    }
    if (current.a === active) return;
    if (active) active.classList.remove('active');
    active = current.a;
    active.classList.add('active');

    // The sidebar scrolls on its own once the list outgrows it.
    var link = active.getBoundingClientRect();
    var box = toc.getBoundingClientRect();
    if (link.top < box.top || link.bottom > box.bottom) {
      active.scrollIntoView({ block: 'nearest' });
    }
  }

  var queued = false;
  function onScroll() {
    if (queued) return;
    queued = true;
    requestAnimationFrame(function () {
      queued = false;
      update();
    });
  }

  addEventListener('scroll', onScroll, { passive: true });
  addEventListener('resize', onScroll);
  update();
})();
