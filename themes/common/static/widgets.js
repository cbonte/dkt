// The components of the widgets, built from the blocks that a `widget` annotation wraps.
// Without this script the blocks stay as they are written and the page reads the same.
(function () {
  "use strict";

  var prefix = "dkt:" + location.pathname + ":";
  var script = document.currentScript && document.currentScript.src;

  function load(id) {
    try {
      return JSON.parse(localStorage.getItem(prefix + id));
    } catch (e) {
      return null;
    }
  }

  function save(id, value) {
    try {
      localStorage.setItem(prefix + id, JSON.stringify(value));
    } catch (e) {}
  }

  function make(tag, className, text) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    if (text) node.textContent = text;
    return node;
  }

  function button(text, onclick) {
    var node = make("button", "dkt-button", text);
    node.type = "button";
    node.addEventListener("click", onclick);
    return node;
  }

  function progress() {
    var line = make("p", "dkt-progress");
    var bar = make("progress");
    var label = make("span");
    label.setAttribute("role", "status");
    line.appendChild(bar);
    line.appendChild(label);
    return {
      node: line,
      set: function (done, total) {
        bar.max = total;
        bar.value = done;
        label.textContent = done + "/" + total;
      },
    };
  }

  // The button that copies a code block.
  function copyButtons() {
    var blocks = document.querySelectorAll("pre");
    Array.prototype.forEach.call(blocks, function (pre) {
      var code = pre.querySelector("code");
      if (!code || pre.querySelector(".dkt-copy") || pre.classList.contains("dkt-screen")) return;
      var copy = make("button", "dkt-copy", "Copy");
      copy.type = "button";
      copy.setAttribute("aria-label", "Copy the code");
      copy.addEventListener("click", function () {
        var text = code.textContent;
        var done = function () {
          copy.textContent = "Copied";
          copy.setAttribute("data-done", "");
          setTimeout(function () {
            copy.textContent = "Copy";
            copy.removeAttribute("data-done");
          }, 1500);
        };
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(text).then(done, function () {});
        } else {
          var area = make("textarea");
          area.value = text;
          document.body.appendChild(area);
          area.select();
          try {
            document.execCommand("copy");
            done();
          } catch (e) {}
          document.body.removeChild(area);
        }
      });
      pre.classList.add("dkt-copyable");
      pre.appendChild(copy);
    });
  }

  // The tabs of a group, which all show the label that the reader chose in one of them.
  var groups = {};

  function readGroup(name) {
    try {
      return localStorage.getItem("dkt-group:" + name);
    } catch (e) {
      return null;
    }
  }

  function joinGroup(name, labels, select) {
    (groups[name] = groups[name] || []).push({ labels: labels, select: select });
  }

  function chooseInGroup(name, label, source) {
    try {
      localStorage.setItem("dkt-group:" + name, label);
    } catch (e) {}
    (groups[name] || []).forEach(function (member) {
      var at = member.labels.indexOf(label);
      if (member.select !== source && at >= 0) member.select(at, false, true);
    });
  }

  // One tab for each heading of the shallowest level, with the blocks that follow it.
  function tabs(root, id) {
    var children = Array.prototype.slice.call(root.children);
    var level = 7;
    children.forEach(function (node) {
      var m = /^H([1-6])$/.exec(node.tagName);
      if (m) level = Math.min(level, +m[1]);
    });
    if (level === 7) return false;
    var groups = [];
    var first = null;
    children.forEach(function (node) {
      if (node.tagName === "H" + level) {
        groups.push({ heading: node, blocks: [] });
        if (!first) first = node;
      } else if (groups.length) {
        groups[groups.length - 1].blocks.push(node);
      }
    });
    var list = make("div", "dkt-tablist");
    list.setAttribute("role", "tablist");
    root.insertBefore(list, first);
    var tabsList = [];
    var panels = groups.map(function (group, i) {
      var panel = make("div", "dkt-panel");
      panel.id = "dkt-" + id + "-panel-" + i;
      panel.setAttribute("role", "tabpanel");
      panel.setAttribute("aria-labelledby", "dkt-" + id + "-tab-" + i);
      root.insertBefore(panel, group.heading);
      group.heading.classList.add("dkt-sr");
      panel.appendChild(group.heading);
      group.blocks.forEach(function (node) {
        panel.appendChild(node);
      });
      var link = group.heading.querySelector("a");
      var tab = make("button", "dkt-tab", (link || group.heading).textContent.trim());
      tab.type = "button";
      tab.id = "dkt-" + id + "-tab-" + i;
      tab.setAttribute("role", "tab");
      tab.setAttribute("aria-controls", panel.id);
      tab.addEventListener("click", function () {
        select(i, false);
      });
      tab.addEventListener("keydown", function (event) {
        var to = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: panels.length - 1 }[event.key];
        if (to === undefined) return;
        event.preventDefault();
        select((to + panels.length) % panels.length, true);
      });
      list.appendChild(tab);
      tabsList.push(tab);
      return panel;
    });
    var group = root.getAttribute("data-group");
    var labels = tabsList.map(function (tab) {
      return tab.textContent;
    });
    // `quiet` is a choice that another group of the same name made, which is not told again.
    function select(index, focus, quiet) {
      panels.forEach(function (panel, i) {
        var on = i === index;
        panel.hidden = !on;
        tabsList[i].setAttribute("aria-selected", on ? "true" : "false");
        tabsList[i].tabIndex = on ? 0 : -1;
      });
      if (focus) tabsList[index].focus();
      save(id, index);
      if (group && !quiet) chooseInGroup(group, labels[index], select);
    }
    var start = load(id);
    var chosen = group ? labels.indexOf(readGroup(group)) : -1;
    if (chosen >= 0) start = chosen;
    var target = location.hash && document.getElementById(decodeURIComponent(location.hash.slice(1)));
    panels.forEach(function (panel, i) {
      if (target && panel.contains(target)) start = i;
    });
    if (group) joinGroup(group, labels, select);
    select(typeof start === "number" && start < panels.length ? start : 0, false, true);
    return true;
  }

  // The boxes of a list of tasks, which the page lets the reader tick.
  function checklist(root, id) {
    var boxes = Array.prototype.slice.call(root.querySelectorAll('input[type="checkbox"]'));
    if (!boxes.length) return false;
    var bar = progress();
    root.insertBefore(bar.node, root.firstChild);
    var saved = load(id) || [];
    function update() {
      var done = boxes.filter(function (box) {
        return box.checked;
      });
      bar.set(done.length, boxes.length);
      save(
        id,
        boxes.map(function (box) {
          return box.checked;
        })
      );
    }
    boxes.forEach(function (box, i) {
      box.disabled = false;
      if (saved[i] !== undefined) box.checked = !!saved[i];
      box.addEventListener("change", update);
    });
    update();
    return true;
  }

  // The items of an ordered list, done one after the other.
  function steps(root, id) {
    var list = root.querySelector("ol");
    if (!list) return false;
    var items = Array.prototype.slice.call(list.children).filter(function (node) {
      return node.tagName === "LI";
    });
    var bar = progress();
    root.insertBefore(bar.node, list);
    var done = load(id) || [];
    var buttons = items.map(function (item, i) {
      var toggle = button("", function () {
        done[i] = !done[i];
        update();
        if (done[i] && buttons[i + 1]) {
          items[i + 1].scrollIntoView({ block: "nearest" });
          buttons[i + 1].focus();
        }
      });
      item.appendChild(toggle);
      return toggle;
    });
    function update() {
      var current = -1;
      var count = 0;
      items.forEach(function (item, i) {
        item.classList.toggle("dkt-done", !!done[i]);
        if (done[i]) count++;
        else if (current < 0) current = i;
      });
      items.forEach(function (item, i) {
        item.classList.toggle("dkt-current", i === current);
        buttons[i].textContent = done[i] ? "Undo" : "Mark as done";
      });
      bar.set(count, items.length);
      save(id, done);
    }
    update();
    return true;
  }

  // The items of a list are the choices. What follows the list shows once one is chosen.
  function quiz(root, id) {
    var list = root.querySelector("ul, ol");
    var answer = parseInt(root.getAttribute("data-answer"), 10) - 1;
    if (!list || !(answer >= 0)) return false;
    var choices = Array.prototype.slice.call(list.children).filter(function (node) {
      return node.tagName === "LI";
    });
    if (answer >= choices.length) return false;
    var after = [];
    var block = list;
    while (block.parentNode !== root) block = block.parentNode;
    for (var next = block.nextElementSibling; next; next = next.nextElementSibling) {
      next.classList.add("dkt-after");
      after.push(next);
    }
    var result = make("p", "dkt-result");
    result.setAttribute("role", "status");
    var again = button("Try again", function () {
      show(null);
      save(id, null);
    });
    block.parentNode.insertBefore(result, block.nextSibling);
    block.parentNode.insertBefore(again, result.nextSibling);
    list.setAttribute("role", "radiogroup");
    function show(chosen) {
      choices.forEach(function (choice, i) {
        choice.classList.toggle("dkt-right", chosen !== null && i === answer);
        choice.classList.toggle("dkt-wrong", chosen === i && i !== answer);
        choice.setAttribute("aria-checked", chosen === i ? "true" : "false");
        choice.setAttribute("aria-disabled", chosen === null ? "false" : "true");
      });
      result.textContent = chosen === null ? "" : chosen === answer ? "Correct." : "Not quite.";
      again.hidden = chosen === null;
      after.forEach(function (node) {
        node.hidden = chosen === null;
      });
    }
    choices.forEach(function (choice, i) {
      choice.setAttribute("role", "radio");
      choice.tabIndex = 0;
      function pick() {
        if (choice.getAttribute("aria-disabled") === "true") return;
        show(i);
        save(id, i);
      }
      choice.addEventListener("click", pick);
      choice.addEventListener("keydown", function (event) {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          pick();
        }
      });
    });
    var chosen = load(id);
    show(typeof chosen === "number" && chosen < choices.length ? chosen : null);
    return true;
  }

  var icons = {
    previous: '<path d="M15 5l-7 7 7 7"/>',
    next: '<path d="M9 5l7 7-7 7"/>',
    pause: '<path d="M8 5v14M16 5v14"/>',
    play: '<path d="M8 5l11 7-11 7z" fill="currentColor"/>',
  };

  function icon(name) {
    return (
      '<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false" fill="none" ' +
      'stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">' +
      icons[name] +
      "</svg>"
    );
  }

  function iconButton(name, label, onclick) {
    var node = make("button", "dkt-icon");
    node.type = "button";
    node.setAttribute("aria-label", label);
    node.innerHTML = icon(name);
    node.addEventListener("click", onclick);
    return node;
  }

  var lightbox = null;

  // The dialog that shows a picture larger, made once for the page.
  function showLarger(image) {
    if (!lightbox) {
      lightbox = make("dialog", "dkt-lightbox");
      lightbox.setAttribute("aria-label", "Enlarged picture");
      lightbox.appendChild(make("img"));
      lightbox.addEventListener("click", function () {
        lightbox.close();
      });
      document.body.appendChild(lightbox);
    }
    var big = lightbox.querySelector("img");
    big.src = image.currentSrc || image.src;
    big.alt = image.alt;
    lightbox.showModal();
  }

  // The slides are a row that the page scrolls. The script adds the buttons, the dots, the loop,
  // the automatic scroll and the larger view, which the annotation asks for.
  function carousel(root, id) {
    var track = root.querySelector(".dkt-track");
    if (!track) return false;
    var slides = Array.prototype.slice.call(track.children).filter(function (slide) {
      var empty = !slide.children.length && !slide.textContent.trim();
      if (empty) track.removeChild(slide);
      return !empty;
    });
    var count = slides.length;
    if (count < 2) return false;
    var looping = root.hasAttribute("data-loop");
    var delay = parseInt(root.getAttribute("data-autoplay"), 10) || 0;
    var most = Math.max.apply(null, (root.getAttribute("data-show") || "1").split(" ").map(Number));
    var reduced = window.matchMedia && matchMedia("(prefers-reduced-motion: reduce)").matches;
    var pad = looping ? Math.min(Math.max(3, most), count) : 0;
    var timer = null;
    var held = false;
    var stopped = reduced;
    var settling = null;

    slides.forEach(function (slide, i) {
      slide.setAttribute("role", "group");
      slide.setAttribute("aria-roledescription", "slide");
      slide.setAttribute("aria-label", i + 1 + " of " + count);
    });
    root.setAttribute("role", "region");
    root.setAttribute("aria-roledescription", "carousel");

    if (root.hasAttribute("data-zoom")) {
      Array.prototype.forEach.call(track.querySelectorAll("picture, img"), function (node) {
        if (node.tagName === "IMG" && node.parentNode.tagName === "PICTURE") return;
        var image = node.tagName === "IMG" ? node : node.querySelector("img");
        var zoom = make("button", "dkt-zoom");
        zoom.type = "button";
        zoom.setAttribute("aria-label", "Enlarge: " + (image.alt || "picture"));
        node.parentNode.insertBefore(zoom, node);
        zoom.appendChild(node);
      });
    }

    // The row is the slides, with the last ones in front and the first ones behind, so it never ends.
    function copy(slide) {
      var clone = slide.cloneNode(true);
      clone.setAttribute("aria-hidden", "true");
      Array.prototype.forEach.call(clone.querySelectorAll("button, a"), function (node) {
        node.tabIndex = -1;
      });
      return clone;
    }
    for (var k = 0; k < pad; k++) {
      track.insertBefore(copy(slides[count - 1 - k]), track.firstChild);
      track.appendChild(copy(slides[k]));
    }

    function step() {
      return slides[0].offsetWidth + (parseFloat(getComputedStyle(track).columnGap) || 0);
    }
    function position() {
      return Math.round(track.scrollLeft / step());
    }
    function visible() {
      return Math.max(1, Math.round(track.clientWidth / step()));
    }
    function last() {
      return looping ? count + 2 * pad - 1 : Math.max(0, count - visible());
    }
    function current() {
      return looping ? (((position() - pad) % count) + count) % count : Math.min(position(), count - 1);
    }
    function jump(to) {
      track.style.scrollSnapType = "none";
      track.scrollLeft = to * step();
      requestAnimationFrame(function () {
        requestAnimationFrame(function () {
          track.style.scrollSnapType = "";
        });
      });
    }
    function go(to) {
      track.scrollTo({
        left: Math.max(0, Math.min(last(), to)) * step(),
        behavior: reduced ? "auto" : "smooth",
      });
    }
    function settle() {
      if (looping) {
        var at = position();
        if (at < pad) jump(at + count);
        else if (at >= pad + count) jump(at - count);
      }
      mark();
    }

    var controls = make("div", "dkt-controls");
    var previous = iconButton("previous", "Previous slides", function () {
      halt();
      go(position() - 1);
    });
    var following = iconButton("next", "Next slides", function () {
      halt();
      go(position() + 1);
    });
    var dots = make("span", "dkt-dots");
    var marks = slides.map(function (slide, i) {
      var dot = make("button", "dkt-dot");
      dot.type = "button";
      dot.setAttribute("aria-label", "Show slides from " + (i + 1));
      dot.addEventListener("click", function () {
        halt();
        go(pad + i);
      });
      dots.appendChild(dot);
      return dot;
    });
    var toggle = null;
    controls.appendChild(previous);
    controls.appendChild(dots);
    controls.appendChild(following);
    if (delay) {
      toggle = make("button", "dkt-icon dkt-toggle");
      toggle.type = "button";
      toggle.innerHTML = icon("pause") + icon("play");
      toggle.addEventListener("click", function () {
        if (stopped) {
          stopped = false;
          play();
        } else {
          halt();
        }
      });
      controls.appendChild(toggle);
    }
    root.appendChild(controls);

    function mark() {
      var at = current();
      marks.forEach(function (dot, i) {
        dot.setAttribute("aria-current", i === at ? "true" : "false");
      });
      previous.disabled = !looping && position() <= 0;
      following.disabled = !looping && position() >= last();
    }
    function label(playing) {
      if (!toggle) return;
      toggle.setAttribute("aria-label", playing ? "Pause the automatic scroll" : "Play the automatic scroll");
      toggle.setAttribute("data-state", playing ? "playing" : "paused");
    }
    function tick() {
      if (held || document.hidden) return;
      go(!looping && position() >= last() ? 0 : position() + 1);
    }
    function play() {
      if (!delay || timer || stopped) return;
      timer = setInterval(tick, delay);
      label(true);
    }
    function halt() {
      stopped = true;
      clearInterval(timer);
      timer = null;
      label(false);
    }

    track.addEventListener(
      "scroll",
      function () {
        mark();
        clearTimeout(settling);
        settling = setTimeout(settle, 140);
      },
      { passive: true }
    );
    track.addEventListener("keydown", function (event) {
      if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") return;
      halt();
      go(position() + (event.key === "ArrowRight" ? 1 : -1));
      event.preventDefault();
    });
    track.addEventListener("pointerdown", halt);
    root.addEventListener("mouseenter", function () {
      held = true;
    });
    root.addEventListener("mouseleave", function () {
      held = false;
    });
    root.addEventListener("focusin", function () {
      held = true;
    });
    root.addEventListener("focusout", function () {
      held = false;
    });
    track.addEventListener("click", function (event) {
      var zoom = event.target.closest(".dkt-zoom");
      if (!zoom) return;
      halt();
      showLarger(zoom.querySelector("img"));
    });
    window.addEventListener("resize", function () {
      jump(pad + current());
      mark();
    });

    root.classList.add("dkt-ready");
    jump(pad);
    mark();
    label(!stopped);
    play();
    return true;
  }

  var builders = { tabs: tabs, checklist: checklist, steps: steps, quiz: quiz, carousel: carousel };

  // The widgets that another file builds, which a page loads only when it holds one.
  window.dkt = {
    builders: builders,
    make: make,
    button: button,
    load: load,
    save: save,
    progress: progress,
    script: script,
    copyButtons: copyButtons,
  };

  function build(root) {
    var build = builders[root.getAttribute("data-widget")];
    try {
      if (build && build(root, root.getAttribute("data-id"))) root.classList.add("dkt-ready");
    } catch (e) {
      if (window.console) console.error(e);
    }
  }

  function start() {
    copyButtons();
    var later = [];
    Array.prototype.forEach.call(document.querySelectorAll(".dkt-widget"), function (root) {
      if (builders[root.getAttribute("data-widget")]) build(root);
      else later.push(root);
    });
    if (!later.length || !script) return;
    var extra = document.createElement("script");
    extra.src = script.replace(/widgets\.js(\?.*)?$/, "widgets-extra.js");
    extra.onload = function () {
      later.forEach(build);
      copyButtons();
    };
    document.head.appendChild(extra);
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
