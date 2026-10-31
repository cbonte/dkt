// The switch between the light and the dark theme. The choice is kept by the browser, and without
// one the system decides.
(function () {
  var KEY = "theme";
  var root = document.documentElement;
  var query = matchMedia("(prefers-color-scheme: dark)");
  var button = document.querySelector(".theme-toggle");
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
    if (!button) return;
    var dark = resolved() === "dark";
    button.setAttribute("aria-label", dark ? "Switch to the light theme" : "Switch to the dark theme");
    button.setAttribute("aria-pressed", String(dark));
  }
  label();
  if (button) {
    button.addEventListener("click", function () {
      var next = resolved() === "dark" ? "light" : "dark";
      root.dataset.theme = next;
      try {
        localStorage.setItem(KEY, next);
      } catch (e) {}
      label();
    });
  }
  query.addEventListener("change", label);

  // The current heading in the list on the right is the last one that has passed under the top
  // bar, so a long section stays current while its content scrolls by.
  var links = Array.prototype.slice.call(document.querySelectorAll(".toc a"));
  if (!links.length) return;
  var targets = links.map(function (a) {
    return document.getElementById(decodeURIComponent(a.getAttribute("href").slice(1)));
  });
  // The titles of level 3 fold under the title of level 2 that holds them. Without this script the
  // list stays open.
  var groups = [];
  var group = null;
  links.forEach(function (a) {
    if (a.classList.contains("l2")) {
      group = { head: a, kids: [], open: false, manual: false, button: null };
      groups.push(group);
    } else if (a.classList.contains("l3") && group) {
      group.kids.push(a);
    }
  });
  function show(g, open) {
    g.open = open;
    g.kids.forEach(function (k) {
      k.hidden = !open;
    });
    if (g.button) {
      g.button.setAttribute("aria-expanded", String(open));
      g.button.setAttribute("aria-label", (open ? "Hide" : "Show") + " the sections of " + g.head.textContent);
    }
  }
  groups.forEach(function (g) {
    var row = document.createElement("div");
    row.className = "toc-row";
    g.head.parentNode.insertBefore(row, g.head);
    if (g.kids.length) {
      g.button = document.createElement("button");
      g.button.type = "button";
      g.button.className = "toc-toggle";
      g.button.addEventListener("click", function () {
        g.manual = true;
        show(g, !g.open);
      });
      row.appendChild(g.button);
    } else {
      var gap = document.createElement("span");
      gap.className = "toc-gap";
      row.appendChild(gap);
    }
    row.appendChild(g.head);
    show(g, false);
  });
  // The group of the current title is open. The others close, unless the reader opened them.
  function openFor(link) {
    groups.forEach(function (g) {
      var holds = g.head === link || g.kids.indexOf(link) >= 0;
      if (holds || !g.manual) show(g, holds);
    });
  }
  var LINE = 90;
  var current = null;
  function update() {
    var found = 0;
    for (var k = 0; k < targets.length; k++) {
      if (targets[k] && targets[k].getBoundingClientRect().top <= LINE) found = k;
    }
    // At the end of the page the last heading may never reach the line.
    if (window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2) found = targets.length - 1;
    if (found === current) return;
    current = found;
    links.forEach(function (a, k) {
      if (k === found) a.setAttribute("aria-current", "true");
      else a.removeAttribute("aria-current");
    });
    openFor(links[found]);
    // The list scrolls inside its own box, so the current heading is kept in view there.
    var box = links[found].closest(".toc");
    var top = links[found].offsetTop;
    if (top < box.scrollTop + 40) box.scrollTop = Math.max(0, top - 40);
    else if (top + links[found].offsetHeight > box.scrollTop + box.clientHeight - 40) box.scrollTop = top + links[found].offsetHeight - box.clientHeight + 40;
  }
  var waiting = false;
  function schedule() {
    if (waiting) return;
    waiting = true;
    requestAnimationFrame(function () {
      waiting = false;
      update();
    });
  }
  addEventListener("scroll", schedule, { passive: true });
  addEventListener("resize", schedule);
  addEventListener("hashchange", schedule);
  update();
})();

// A heading with an id shows a link on hover, and a click on it copies the address of the section.
(function () {
  var headings = document.querySelectorAll(".prose h2[id], .prose h3[id], .prose h4[id], .prose h5[id], .prose h6[id]");
  headings.forEach(function (h) {
    var button = document.createElement("button");
    button.type = "button";
    button.className = "heading-link";
    button.setAttribute("aria-label", "Copy the link to this section");
    button.title = "Copy the link to this section";
    button.textContent = "#";
    button.addEventListener("click", function () {
      var url = location.href.split("#")[0] + "#" + h.id;
      history.replaceState(null, "", "#" + h.id);
      var done = function () {
        button.title = "Copied";
        button.classList.add("copied");
        setTimeout(function () {
          button.title = "Copy the link to this section";
          button.classList.remove("copied");
        }, 1500);
      };
      // A page served without a secure connection has no clipboard API, so a hidden field is used.
      var fallback = function () {
        var field = document.createElement("textarea");
        field.value = url;
        field.setAttribute("readonly", "");
        field.style.position = "fixed";
        field.style.opacity = "0";
        document.body.appendChild(field);
        field.select();
        var ok = false;
        try {
          ok = document.execCommand("copy");
        } catch (e) {}
        document.body.removeChild(field);
        if (ok) done();
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(url).then(done, fallback);
      } else {
        fallback();
      }
    });
    h.appendChild(button);
  });
})();
