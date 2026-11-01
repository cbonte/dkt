// The selector of versions and the notes of a version of a documentation.
//
// The page names the list of the versions in `meta name="dkt-versions"`.
// The script reads that list once, rebuilds the selector from it and shows the warning of the version.
//
// A choice leads to the same page and anchor in the target version when its `version.json` lists them.
// The page or the anchor that the target lacks is told in a note on the page that opens.
// Without the script, the links of the build lead to the same page in the versions known then.
(function () {
  "use strict";

  var NOTE_KEY = "dkt-versions-note";
  var core = {};

  // The path of the page inside its version, such as `guide/x/`, or null outside the version.
  core.inside = function (pathname, root, name) {
    var head = root + "/" + name + "/";
    if (pathname.indexOf(head) !== 0) return null;
    var rel = pathname.slice(head.length);
    if (/(^|\/)index\.html$/.test(rel)) rel = rel.slice(0, rel.length - "index.html".length);
    return rel;
  };

  // The address to open in `target`, and what the target lacks: `anchor`, `page` or null.
  core.choose = function (rel, hash, root, target, pages) {
    var home = root + "/" + target + "/";
    if (rel === null) return { url: home, lost: null };
    if (!pages || !Object.prototype.hasOwnProperty.call(pages, rel)) {
      return { url: home, lost: rel === "" ? null : "page" };
    }
    var url = home + rel;
    var id = (hash || "").replace(/^#/, "");
    if (!id) return { url: url, lost: null };
    var decoded = id;
    try {
      decoded = decodeURIComponent(id);
    } catch (e) {}
    if (pages[rel].indexOf(decoded) >= 0) return { url: url + "#" + id, lost: null };
    return { url: url, lost: "anchor" };
  };

  // The warning of the version `name` in `list`: `dev`, `deprecated`, `newer`, or null.
  core.warning = function (list, name) {
    var found = null;
    (list.versions || []).forEach(function (v) {
      if (v.name === name) found = v;
    });
    if (!found) return undefined;
    if (found.status === "dev") return "dev";
    if (found.status === "deprecated") return "deprecated";
    if (list.latest && list.latest !== name) return "newer";
    return null;
  };

  // The label of a version, with its state written as text.
  core.label = function (v, latest) {
    if (v.name === latest) return v.title + " (latest)";
    if (v.status === "dev") return v.title + " (development)";
    if (v.status === "deprecated") return v.title + " (deprecated)";
    return v.title;
  };

  // The sentence of a warning, before the link to the latest release.
  core.sentence = function (kind, title) {
    if (kind === "dev") return "This is the documentation of " + title + ", a development version that is not released.";
    if (kind === "deprecated") return "This is the documentation of " + title + ", a deprecated version.";
    if (kind === "newer") return "This is the documentation of " + title + ", and a newer version exists.";
    return "";
  };

  if (typeof module === "object" && module.exports) {
    module.exports = core;
    return;
  }

  var meta = document.querySelector('meta[name="dkt-versions"]');
  if (!meta || !window.fetch) return;
  var root = meta.getAttribute("data-root") || "";
  var name = meta.getAttribute("data-current") || "";
  var rel = core.inside(location.pathname, root, name);
  var manifests = {};

  function make(tag, className, text) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    if (text) node.textContent = text;
    return node;
  }

  function json(url) {
    return fetch(url).then(function (r) {
      if (!r.ok) throw new Error(url);
      return r.json();
    });
  }

  // The place of a note: after the warning when there is one, at the start of the content otherwise.
  function place(node) {
    var after = document.querySelector(".dkt-version-note");
    if (after && after !== node) {
      after.parentNode.insertBefore(node, after.nextSibling);
      return;
    }
    var main = document.querySelector("main") || document.body;
    main.insertBefore(node, main.firstChild);
  }

  function linkTo(v, known) {
    var a = make("a", null, null);
    a.href = known[v.name] || v.url;
    a.setAttribute("data-version", v.name);
    return a;
  }

  // The selector holds the versions of the list in its order, and keeps the links of the build.
  function rebuild(list) {
    var ul = document.querySelector("details.dkt-versions ul");
    if (!ul) return {};
    var known = {};
    Array.prototype.forEach.call(ul.querySelectorAll("a[data-version]"), function (a) {
      known[a.getAttribute("data-version")] = a.getAttribute("href");
    });
    var versions = (list.versions || []).slice();
    if (!versions.some(function (v) { return v.name === name; })) return known;
    while (ul.firstChild) ul.removeChild(ul.firstChild);
    versions.forEach(function (v) {
      var a = linkTo(v, known);
      a.textContent = core.label(v, list.latest);
      if (v.name === name) a.setAttribute("aria-current", "true");
      var li = make("li", null, null);
      li.appendChild(a);
      ul.appendChild(li);
    });
    versions.forEach(function (v) {
      if (!known[v.name]) known[v.name] = v.url;
    });
    return known;
  }

  // The warning follows the list as it is now, so a page built as the latest learns of a newer one.
  function warn(list, known) {
    var kind = core.warning(list, name);
    if (kind === undefined) return;
    var note = document.querySelector(".dkt-version-note");
    if (!kind) {
      if (note) note.hidden = true;
      return;
    }
    if (!note) {
      note = make("div", "dkt-version-note", null);
      note.setAttribute("role", "note");
      place(note);
    }
    var current = null;
    var latest = null;
    list.versions.forEach(function (v) {
      if (v.name === name) current = v;
      if (v.name === list.latest) latest = v;
    });
    while (note.firstChild) note.removeChild(note.firstChild);
    note.appendChild(document.createTextNode(core.sentence(kind, current.title)));
    if (latest) {
      note.appendChild(document.createTextNode(" "));
      var a = linkTo(latest, known);
      a.textContent = "Read it in " + latest.title;
      note.appendChild(a);
      note.appendChild(document.createTextNode("."));
    }
    note.hidden = false;
  }

  // The note left by the page of another version, shown once.
  function moved() {
    var text = null;
    try {
      text = sessionStorage.getItem(NOTE_KEY);
      sessionStorage.removeItem(NOTE_KEY);
    } catch (e) {}
    if (!text) return;
    var note = make("div", "dkt-version-moved", null);
    note.setAttribute("role", "status");
    note.appendChild(make("span", null, text));
    var close = make("button", null, "Close");
    close.type = "button";
    close.addEventListener("click", function () {
      note.hidden = true;
    });
    note.appendChild(close);
    place(note);
  }

  function remember(lost, title) {
    var text;
    if (lost === "anchor") {
      var id = location.hash.replace(/^#/, "");
      try {
        id = decodeURIComponent(id);
      } catch (e) {}
      var heading = document.getElementById(id);
      var what = heading ? heading.textContent.trim() : id;
      text = "“" + what + "” is not in the documentation of " + title + ".";
    } else {
      text = "This page is not in the documentation of " + title + ".";
    }
    try {
      sessionStorage.setItem(NOTE_KEY, text);
    } catch (e) {}
  }

  function go(a) {
    var target = a.getAttribute("data-version");
    var title = target;
    var listed = document.querySelector('details.dkt-versions a[data-version="' + target + '"]');
    if (listed) title = listed.textContent.replace(/ \((latest|development|deprecated)\)$/, "");
    var pages = manifests[target]
      ? Promise.resolve(manifests[target])
      : json(root + "/" + target + "/version.json").then(function (m) {
          manifests[target] = m;
          return m;
        });
    pages
      .then(function (m) {
        var pick = core.choose(rel, location.hash, root, target, m.pages);
        if (pick.lost) remember(pick.lost, title);
        location.href = pick.url;
      })
      .catch(function () {
        location.href = a.href;
      });
  }

  document.addEventListener("click", function (e) {
    var a = e.target.closest && e.target.closest("a[data-version]");
    if (!a || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    if (a.getAttribute("data-version") === name) return;
    e.preventDefault();
    go(a);
  });

  document.addEventListener("keydown", function (e) {
    if (e.key !== "Escape") return;
    var open = document.querySelector("details.dkt-versions[open]");
    if (open) {
      open.open = false;
      open.querySelector("summary").focus();
      return;
    }
    var note = document.querySelector(".dkt-version-moved:not([hidden])");
    if (note) note.hidden = true;
  });

  moved();
  json(meta.getAttribute("content"))
    .then(function (list) {
      var known = rebuild(list);
      warn(list, known);
    })
    .catch(function () {});
})();
