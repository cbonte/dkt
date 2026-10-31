// The switch between the light and the dark theme. The choice is kept by the browser, and without
// one the system decides.
(function () {
  var KEY = "theme";
  var root = document.documentElement;
  var query = matchMedia("(prefers-color-scheme: dark)");
  var button = document.querySelector(".theme-toggle");
  if (!button) return;
  function stored() {
    try {
      var t = localStorage.getItem(KEY);
      return t === "light" || t === "dark" ? t : null;
    } catch (e) {
      return null;
    }
  }
  function resolved() {
    return root.dataset.theme || (query.matches ? "dark" : "light");
  }
  function label() {
    var dark = resolved() === "dark";
    button.setAttribute("aria-label", dark ? "Switch to the light theme" : "Switch to the dark theme");
    button.setAttribute("aria-pressed", String(dark));
  }
  label();
  button.addEventListener("click", function () {
    var next = resolved() === "dark" ? "light" : "dark";
    root.dataset.theme = next;
    try {
      localStorage.setItem(KEY, next);
    } catch (e) {}
    label();
  });
  query.addEventListener("change", function () {
    if (!stored()) label();
  });
})();
