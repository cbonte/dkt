// The search palette. It opens from the button, from the slash key and from Ctrl+K, reads the index
// of titles that the build wrote, and lists the pages and the sections that match what the reader
// typed.
//
// Matching ignores case and accents, wants every word, in any order, and ranks a word found at the
// start of a word above one found inside it. A title ranks above its aliases, which rank above the
// page and the section it sits under, then the description. A word that is not found at all can
// still match a title by one slip of the keys or by its letters in order. A query that starts with
// the hash sign looks at sections only, and one that starts with the greater-than sign at pages.
// A result in the same part of the site as the page being read ranks above the others.
(function () {
  "use strict";
  var LIMIT = 20;
  var RECENT = "search-recent";

  function fold(text) {
    // The folded text, and for each of its characters the position of the character it came from.
    var out = "";
    var map = [];
    for (var i = 0; i < text.length; i++) {
      var f = text[i].normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
      for (var k = 0; k < f.length; k++) {
        out += f[k];
        map.push(i);
      }
    }
    return { text: out, map: map };
  }
  function prepare(rows) {
    return rows.map(function (r) {
      return {
        t: r.t, p: r.p, u: r.u, l: r.l, b: r.b || "", d: r.d || "", a: r.a || "",
        ft: fold(r.t), fh: fold(r.t.replace(/ \([^()]*\)$/, "")).text, fp: fold(r.p).text, fb: fold(r.b || "").text, fd: fold(r.d || "").text, fa: fold(r.a || "").text,
      };
    });
  }
  function startAt(text, word) {
    // The first place where `word` starts a word of `text`.
    var from = 0;
    for (;;) {
      var i = text.indexOf(word, from);
      if (i < 0) return -1;
      if (i === 0 || !/[a-z0-9]/.test(text[i - 1])) return i;
      from = i + 1;
    }
  }
  function close(a, b) {
    // One slip apart: a letter changed, missing, added, or two neighbours swapped.
    if (a === b) return true;
    if (Math.abs(a.length - b.length) > 1) return false;
    if (a.length === b.length) {
      var bad = [];
      for (var i = 0; i < a.length; i++) if (a[i] !== b[i]) bad.push(i);
      if (bad.length === 1) return true;
      return bad.length === 2 && bad[1] === bad[0] + 1 && a[bad[0]] === b[bad[1]] && a[bad[1]] === b[bad[0]];
    }
    var long = a.length > b.length ? a : b;
    var short = a.length > b.length ? b : a;
    var k = 0;
    while (k < short.length && long[k] === short[k]) k++;
    return long.slice(k + 1) === short.slice(k);
  }
  function slip(row, w) {
    var words = (row.ft.text + " " + row.fa).split(/[^a-z0-9]+/).filter(Boolean);
    return words.some(function (x) {
      return close(w, x) || close(w, x.slice(0, w.length));
    });
  }
  function inOrder(text, w) {
    var at = 0;
    for (var i = 0; i < w.length; i++) {
      at = text.indexOf(w[i], at);
      if (at < 0) return false;
      at++;
    }
    return true;
  }
  function wordScore(row, w) {
    var t = row.ft.text;
    if (t === w) return 100;
    if (row.fh === w) return 90;
    if (startAt(t, w) >= 0) return 40;
    if (t.indexOf(w) >= 0) return 15;
    if (row.fa && startAt(row.fa, w) >= 0) return 30;
    if (row.fa && row.fa.indexOf(w) >= 0) return 12;
    if (row.fb && startAt(row.fb, w) >= 0) return 8;
    if (startAt(row.fp, w) >= 0) return 8;
    if ((row.fb && row.fb.indexOf(w) >= 0) || row.fp.indexOf(w) >= 0) return 3;
    if (row.fd && startAt(row.fd, w) >= 0) return 4;
    if (row.fd && row.fd.indexOf(w) >= 0) return 2;
    if (w.length >= 5 && slip(row, w)) return 6;
    if (w.length >= 4 && inOrder(t, w)) return 2;
    return 0;
  }
  function score(row, words, whole) {
    var s = 0;
    for (var i = 0; i < words.length; i++) {
      var w = wordScore(row, words[i]);
      if (!w) return 0;
      s += w;
    }
    if (row.ft.text.indexOf(whole) === 0) s += 30;
    if (row.l === 1) s += 10;
    return s + (20 - Math.min(row.t.length, 20)) / 2;
  }
  function find(rows, query, here) {
    var q = query.trim();
    var scope = q.charAt(0) === "#" ? "sections" : q.charAt(0) === ">" ? "pages" : "";
    if (scope) q = q.slice(1);
    var words = fold(q.trim()).text.split(/\s+/).filter(Boolean);
    if (!words.length) return [];
    var whole = words.join(" ");
    var found = [];
    rows.forEach(function (row, i) {
      if ((scope === "sections" && row.l === 1) || (scope === "pages" && row.l > 1)) return;
      var s = score(row, words, whole);
      if (s && here && row.u.indexOf(here) === 0) s += 25;
      if (s) found.push({ row: row, s: s, i: i, words: words });
    });
    found.sort(function (a, b) {
      return b.s - a.s || a.i - b.i;
    });
    return found.slice(0, LIMIT);
  }

  var core = { fold: fold, prepare: prepare, find: find, close: close };
  if (typeof module === "object" && module.exports) module.exports = core;
  var dialog = typeof document === "undefined" ? null : document.querySelector("dialog.search");
  if (!dialog) return;

  var input = dialog.querySelector("input");
  var list = dialog.querySelector(".results");
  var status = dialog.querySelector(".status");
  var base = dialog.dataset.base || "";
  var index = null;
  var shown = [];

  function here() {
    // The address of the part of the site that holds the page being read.
    var path = window.location.pathname;
    if (path.indexOf(base + "/") !== 0) return "";
    var rest = path.slice(base.length + 1).split("/")[0];
    return rest ? base + "/" + rest + "/" : "";
  }
  function load() {
    if (!index) {
      index = fetch(base + "/search.json")
        .then(function (r) {
          return r.json();
        })
        .then(prepare);
    }
    return index;
  }
  function recent() {
    try {
      var saved = JSON.parse(localStorage.getItem(RECENT) || "[]");
      return Array.isArray(saved) ? saved : [];
    } catch (e) {
      return [];
    }
  }
  function remember(row) {
    var kept = recent().filter(function (r) {
      return r.u !== row.u;
    });
    kept.unshift({ t: row.t, p: row.p, u: row.u, l: row.l, b: row.b, d: row.d });
    try {
      localStorage.setItem(RECENT, JSON.stringify(kept.slice(0, 6)));
    } catch (e) {}
  }
  function marked(parent, hit) {
    var row = hit.row;
    var spans = [];
    hit.words.forEach(function (w) {
      var from = 0;
      for (;;) {
        var i = row.ft.text.indexOf(w, from);
        if (i < 0) break;
        spans.push([row.ft.map[i], row.ft.map[i + w.length - 1] + 1]);
        from = i + w.length;
      }
    });
    spans.sort(function (a, b) {
      return a[0] - b[0];
    });
    var at = 0;
    spans.forEach(function (s) {
      if (s[1] <= at) return;
      var start = Math.max(s[0], at);
      if (start > at) parent.appendChild(document.createTextNode(row.t.slice(at, start)));
      var m = document.createElement("mark");
      m.textContent = row.t.slice(start, s[1]);
      parent.appendChild(m);
      at = s[1];
    });
    if (at < row.t.length) parent.appendChild(document.createTextNode(row.t.slice(at)));
  }
  function line(hit, n) {
    var row = hit.row;
    var a = document.createElement("a");
    a.href = row.u;
    a.id = "hit-" + n;
    a.dataset.at = String(n);
    a.setAttribute("role", "option");
    var b = document.createElement("b");
    marked(b, hit);
    a.appendChild(b);
    var under = row.l === 1 ? row.d : row.b ? row.p + " › " + row.b : row.p;
    if (under) {
      var s = document.createElement("span");
      s.textContent = under;
      a.appendChild(s);
    }
    return a;
  }
  function group(name) {
    var h = document.createElement("div");
    h.className = "group";
    h.textContent = name;
    list.appendChild(h);
  }
  function show(groups) {
    list.textContent = "";
    shown = [];
    groups.forEach(function (g) {
      if (!g[1].length) return;
      group(g[0]);
      g[1].forEach(function (h) {
        list.appendChild(line(h, shown.length));
        shown.push(h.row);
      });
    });
    var first = list.querySelector("a");
    if (first) {
      first.setAttribute("aria-selected", "true");
      input.setAttribute("aria-activedescendant", first.id);
    }
  }
  function idle(rows) {
    // Nothing typed: what was opened from here lately, else the pages.
    var kept = prepare(recent()).map(function (row) {
      return { row: row, words: [] };
    });
    if (kept.length) {
      status.textContent = "";
      show([["Recent", kept]]);
      return;
    }
    var pages = rows
      .filter(function (r) {
        return r.l === 1;
      })
      .slice(0, 8)
      .map(function (row) {
        return { row: row, words: [] };
      });
    status.textContent = "Type to search the titles.";
    show([["Pages", pages]]);
  }
  function search() {
    var q = input.value;
    load()
      .then(function (rows) {
        if (!q.trim()) return idle(rows);
        var found = find(rows, q, here());
        status.textContent = found.length ? "" : "No title holds " + q.trim() + ".";
        show([
          ["Pages", found.filter(function (h) { return h.row.l === 1; })],
          ["Sections", found.filter(function (h) { return h.row.l > 1; })],
        ]);
      })
      .catch(function () {
        status.textContent = "The search is not available.";
      });
  }
  function active(to) {
    var rows = list.querySelectorAll("a");
    if (!rows.length) return;
    var now = list.querySelector("a[aria-selected=true]");
    var at = now ? Array.prototype.indexOf.call(rows, now) : -1;
    var next = (at + to + rows.length) % rows.length;
    if (now) now.removeAttribute("aria-selected");
    rows[next].setAttribute("aria-selected", "true");
    rows[next].scrollIntoView({ block: "nearest" });
    input.setAttribute("aria-activedescendant", rows[next].id);
  }
  function open() {
    if (dialog.open) return;
    dialog.showModal();
    input.select();
    search();
  }

  document.querySelectorAll(".search-open").forEach(function (b) {
    b.hidden = false;
    b.addEventListener("click", open);
  });
  document.addEventListener("keydown", function (e) {
    var typing = /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName || "") || e.target.isContentEditable;
    if ((e.key === "/" && !typing && !e.ctrlKey && !e.metaKey) || ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k")) {
      e.preventDefault();
      open();
    }
  });
  input.addEventListener("input", search);
  input.addEventListener("keydown", function (e) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      active(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active(-1);
    } else if (e.key === "Enter") {
      var now = list.querySelector("a[aria-selected=true]");
      if (now) {
        e.preventDefault();
        remember(shown[Number(now.dataset.at)]);
        window.location.href = now.href;
      }
    }
  });
  list.addEventListener("click", function (e) {
    var a = e.target.closest("a");
    if (a) remember(shown[Number(a.dataset.at)]);
  });
  dialog.addEventListener("click", function (e) {
    if (e.target === dialog) dialog.close();
  });
  dialog.addEventListener("close", function () {
    input.removeAttribute("aria-activedescendant");
  });
  var from = new URLSearchParams(window.location.search).get("q");
  if (from) {
    open();
    input.value = from;
    search();
  }
})();
