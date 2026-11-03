(function () {
  const root = document.documentElement;
  let stored = null;
  try { stored = window.localStorage.getItem("slopsearx-theme"); } catch (_) { /* storage is optional */ }
  const configured = root.dataset.defaultTheme || "dark";
  const allowed = ["dark", "darker"];
  const theme = allowed.includes(stored) ? stored : (allowed.includes(configured) ? configured : "dark");
  root.dataset.theme = theme;
  const button = document.querySelector("[data-theme-toggle]");
  const update = function () {
    const darker = root.dataset.theme === "darker";
    if (button) {
      button.textContent = darker ? "Dark mode" : "Darker mode";
      button.setAttribute("aria-label", darker ? "Use dark mode" : "Use darker mode");
      button.setAttribute("aria-pressed", String(darker));
    }
  };
  update();
  if (button) button.addEventListener("click", function () {
    root.dataset.theme = root.dataset.theme === "darker" ? "dark" : "darker";
    try { window.localStorage.setItem("slopsearx-theme", root.dataset.theme); } catch (_) { /* storage is optional */ }
    update();
  });
  const scopePanel = document.querySelector(".scope-panel");
  if (scopePanel && window.matchMedia && window.matchMedia("(max-width: 900px)").matches) scopePanel.removeAttribute("open");
  const input = document.querySelector(".search-input");
  const resultCards = Array.from(document.querySelectorAll("[data-result-card]"));
  let activeResult = -1;
  const reducedMotion = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const setActiveResult = function (next) {
    if (!resultCards.length) return;
    activeResult = Math.max(0, Math.min(resultCards.length - 1, next));
    resultCards.forEach(function (card, index) {
      card.classList.toggle("is-active", index === activeResult);
    });
    resultCards[activeResult].scrollIntoView({ block: "nearest", behavior: reducedMotion ? "auto" : "smooth" });
    const link = resultCards[activeResult].querySelector(".result-link");
    if (link) link.focus({ preventScroll: true });
  };
  document.addEventListener("keydown", function (event) {
    const target = event.target;
    const typing = target && (target.matches("input, textarea, select") || target.isContentEditable);
    if (((event.key === "/" && !typing) || (event.key === "k" && (event.metaKey || event.ctrlKey))) && document.activeElement !== input) {
      event.preventDefault(); if (input) input.focus();
    } else if (!typing && event.key === "j" && resultCards.length) {
      event.preventDefault(); setActiveResult(activeResult + 1);
    } else if (!typing && event.key === "k" && resultCards.length) {
      event.preventDefault(); setActiveResult(activeResult - 1);
    }
    // j/k focus a real link. Enter keeps the browser's native activation of
    // whichever link, summary, button or form control currently owns focus.
  });
  document.querySelectorAll('form[action="/search"]').forEach(function (form) {
    form.addEventListener("submit", function () {
      form.setAttribute("aria-busy", "true");
      const status = document.querySelector("[data-submit-status]");
      if (status) status.textContent = "Searching configured sources";
    });
  });
  window.addEventListener("pageshow", function () {
    document.querySelectorAll('form[aria-busy="true"]').forEach(function (form) { form.removeAttribute("aria-busy"); });
    const status = document.querySelector("[data-submit-status]");
    if (status) status.textContent = "";
  });
  const invalid = document.body.dataset.invalidField;
  if (invalid) {
    const name = invalid === "query" ? "q" : invalid;
    const control = Array.from(document.querySelectorAll("input[name],select[name]")).find(function (element) {
      return element.name === name && element.type !== "hidden";
    });
    if (control) {
      control.setAttribute("aria-invalid", "true");
      control.setAttribute("aria-describedby", "portal-error");
      const panel = control.closest("details");
      if (panel) panel.open = true;
      control.focus();
    }
  }
}());
