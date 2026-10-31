// The components of the widgets, built from the blocks that a `widget` annotation wraps.
// Without this script the blocks stay as they are written and the page reads the same.
(function () {
  "use strict";

  var prefix = "dkt:" + location.pathname + ":";

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
      if (!code || pre.querySelector(".dkt-copy")) return;
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
    function select(index, focus) {
      panels.forEach(function (panel, i) {
        var on = i === index;
        panel.hidden = !on;
        tabsList[i].setAttribute("aria-selected", on ? "true" : "false");
        tabsList[i].tabIndex = on ? 0 : -1;
      });
      if (focus) tabsList[index].focus();
      save(id, index);
    }
    var start = load(id);
    var target = location.hash && document.getElementById(decodeURIComponent(location.hash.slice(1)));
    panels.forEach(function (panel, i) {
      if (target && panel.contains(target)) start = i;
    });
    select(typeof start === "number" && start < panels.length ? start : 0, false);
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

  // One slide for each run of blocks between two thematic breaks.
  function carousel(root, id) {
    var children = Array.prototype.slice.call(root.children);
    var slides = [[]];
    children.forEach(function (node) {
      if (node.tagName === "HR") slides.push([]);
      else slides[slides.length - 1].push(node);
    });
    slides = slides.filter(function (blocks) {
      return blocks.length;
    });
    if (slides.length < 2) return false;
    children.forEach(function (node) {
      if (node.tagName === "HR") root.removeChild(node);
    });
    var view = make("div", "dkt-viewport");
    view.tabIndex = 0;
    view.setAttribute("role", "group");
    view.setAttribute("aria-roledescription", "carousel");
    var frames = slides.map(function (blocks, i) {
      var frame = make("div", "dkt-slide");
      frame.setAttribute("role", "group");
      frame.setAttribute("aria-roledescription", "slide");
      frame.setAttribute("aria-label", i + 1 + " of " + slides.length);
      blocks.forEach(function (node) {
        frame.appendChild(node);
      });
      view.appendChild(frame);
      return frame;
    });
    var controls = make("div", "dkt-controls");
    var previous = button("Previous", function () {
      go(index - 1);
    });
    var following = button("Next", function () {
      go(index + 1);
    });
    var dots = make("div", "dkt-dots");
    var marks = frames.map(function (frame, i) {
      var dot = make("button", "dkt-dot");
      dot.type = "button";
      dot.setAttribute("aria-label", "Slide " + (i + 1));
      dot.addEventListener("click", function () {
        go(i);
      });
      dots.appendChild(dot);
      return dot;
    });
    var counter = make("span", "dkt-progress");
    counter.setAttribute("role", "status");
    controls.appendChild(previous);
    controls.appendChild(dots);
    controls.appendChild(counter);
    controls.appendChild(following);
    root.appendChild(view);
    root.appendChild(controls);
    var index = 0;
    function go(to) {
      index = Math.max(0, Math.min(frames.length - 1, to));
      frames.forEach(function (frame, i) {
        frame.hidden = i !== index;
        marks[i].setAttribute("aria-current", i === index ? "true" : "false");
      });
      previous.disabled = index === 0;
      following.disabled = index === frames.length - 1;
      counter.textContent = index + 1 + "/" + frames.length;
    }
    view.addEventListener("keydown", function (event) {
      if (event.key === "ArrowLeft") go(index - 1);
      else if (event.key === "ArrowRight") go(index + 1);
      else return;
      event.preventDefault();
    });
    var from = null;
    view.addEventListener("pointerdown", function (event) {
      from = event.clientX;
    });
    view.addEventListener("pointerup", function (event) {
      if (from !== null && Math.abs(event.clientX - from) > 50) go(index + (event.clientX < from ? 1 : -1));
      from = null;
    });
    go(0);
    return true;
  }

  var builders = { tabs: tabs, checklist: checklist, steps: steps, quiz: quiz, carousel: carousel };

  function start() {
    copyButtons();
    var widgets = document.querySelectorAll(".dkt-widget");
    Array.prototype.forEach.call(widgets, function (root) {
      var build = builders[root.getAttribute("data-widget")];
      try {
        if (build && build(root, root.getAttribute("data-id"))) root.classList.add("dkt-ready");
      } catch (e) {
        if (window.console) console.error(e);
      }
    });
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
