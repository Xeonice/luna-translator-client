(() => {
  'use strict';
  const modeButtons = [...document.querySelectorAll('[data-mode]')];
  const modePanels = [...document.querySelectorAll('[data-panel]')];
  const opacitySlider = document.getElementById('opacity-slider');
  const revealSlider = document.getElementById('reveal-slider');
  const revealStage = document.getElementById('reveal-stage');

  function selectMode(mode) {
    modeButtons.forEach(button => button.setAttribute('aria-pressed', String(button.dataset.mode === mode)));
    modePanels.forEach(panel => { panel.hidden = panel.dataset.panel !== mode; });
  }
  modeButtons.forEach(button => button.addEventListener('click', () => selectMode(button.dataset.mode)));

  function updateOpacity() {
    const value = Math.max(0, Math.min(100, Number(opacitySlider.value)));
    document.getElementById('overlay-image').style.opacity = String(value / 100);
    document.getElementById('opacity-value').value = `${value}%`;
    document.getElementById('stage-opacity').textContent = `${value}%`;
  }
  opacitySlider.addEventListener('input', updateOpacity);
  updateOpacity();

  function updateReveal(value) {
    const normalized = Math.round(Math.max(0, Math.min(100, value)));
    revealSlider.value = String(normalized);
    document.getElementById('reveal-value').value = `${normalized}%`;
    revealStage.style.setProperty('--reveal', `${normalized}%`);
  }
  revealSlider.addEventListener('input', () => updateReveal(Number(revealSlider.value)));
  let dragging = false;
  function revealAtPointer(event) {
    const rect = revealStage.getBoundingClientRect();
    if (rect.width > 0) updateReveal((event.clientX - rect.left) / rect.width * 100);
  }
  revealStage.addEventListener('pointerdown', event => {
    if (event.button !== 0) return;
    dragging = true;
    revealStage.setPointerCapture(event.pointerId);
    revealAtPointer(event);
  });
  revealStage.addEventListener('pointermove', event => { if (dragging) revealAtPointer(event); });
  revealStage.addEventListener('pointerup', event => {
    dragging = false;
    if (revealStage.hasPointerCapture(event.pointerId)) revealStage.releasePointerCapture(event.pointerId);
  });
  revealStage.addEventListener('pointercancel', () => { dragging = false; });
  updateReveal(Number(revealSlider.value));

  document.querySelectorAll('img').forEach(img => {
    img.addEventListener('error', () => {
      img.classList.add('load-error');
      img.closest('.image-card')?.classList.add('has-error');
    });
  });
})();
