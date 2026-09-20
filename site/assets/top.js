// The way back up appears once there is somewhere to come back from. The
// button is plain HTML and stays visible without script; `auto` is what
// hands its visibility over to the scroll position.
(function () {
  var button = document.querySelector('.totop');
  if (!button) return;
  button.classList.add('auto');
  function update() {
    button.classList.toggle('visible', window.pageYOffset > 400);
  }
  addEventListener('scroll', update, { passive: true });
  update();
})();
