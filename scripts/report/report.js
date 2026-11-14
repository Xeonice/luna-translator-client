(() => {
  const theme = document.querySelector('[data-theme-toggle]');
  theme?.addEventListener('click', () => {
    const active = document.body.classList.toggle('dark');
    theme.setAttribute('aria-pressed', String(active));
    theme.textContent = active ? '浅色阅读' : '深色阅读';
  });
  document.querySelector('[data-print]')?.addEventListener('click', () => window.print());
  const headings = [...document.querySelectorAll('.chapter[id]')];
  const links = [...document.querySelectorAll('.sidebar a[href^="#"]')];
  if ('IntersectionObserver' in window) {
    const observer = new IntersectionObserver(entries => {
      const visible = entries.find(entry => entry.isIntersecting);
      if (!visible) return;
      links.forEach(link => link.classList.toggle('active', decodeURIComponent(link.hash.slice(1)) === visible.target.id));
    }, {rootMargin: '-5% 0px -65% 0px'});
    headings.forEach(heading => observer.observe(heading));
  }
  const rows = [...document.querySelectorAll('#requirements-table tbody tr')];
  const query = document.querySelector('#requirement-search');
  const priority = document.querySelector('#requirement-priority');
  const workPackage = document.querySelector('#requirement-package');
  function filterRequirements() {
    const text = (query?.value || '').trim().toLocaleLowerCase();
    const level = priority?.value || '';
    const packet = workPackage?.value || '';
    let visible = 0;
    rows.forEach(row => {
      const matches = (!text || row.textContent.toLocaleLowerCase().includes(text)) &&
        (!level || row.dataset.priority === level) &&
        (!packet || row.dataset.packages.split(';').includes(packet));
      row.hidden = !matches;
      if (matches) visible++;
    });
    const output = document.querySelector('#requirement-count');
    if (output) output.textContent = `显示 ${visible} / ${rows.length} 项`;
    const empty = document.querySelector('#requirement-empty');
    if (empty) empty.hidden = visible > 0;
  }
  [query, priority, workPackage].forEach(input => input?.addEventListener('input', filterRequirements));
  filterRequirements();
  document.querySelectorAll('[data-platform]').forEach(button => {
    button.addEventListener('click', () => {
      const platform = button.dataset.platform;
      document.querySelectorAll('[data-platform]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
      document.querySelectorAll('[data-environment]').forEach(card => card.classList.toggle('dim', platform !== 'all' && card.dataset.environment !== platform));
    });
  });
})();
