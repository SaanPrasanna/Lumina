// Applies the saved theme before the first paint, so a dark window never
// flashes white while React starts (see src/app/theme.ts for the store).
(function () {
  var theme = "system";
  try {
    var saved = JSON.parse(localStorage.getItem("lumina.theme") || "null");
    if (saved && saved.state && saved.state.theme) theme = saved.state.theme;
  } catch {
    /* storage unavailable: follow the system */
  }
  var dark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  if (dark) document.documentElement.classList.add("dark");
})();
