// The widgets that a page loads only when it holds one: a filterable table, a comparison of 2
// pictures, a diff, a chart, flashcards and the others. widgets.js loads this file.
(function () {
  "use strict";

  var dkt = window.dkt;
  if (!dkt) return;
  var make = dkt.make;
  var builders = dkt.builders;

  function all(root, selector) {
    return Array.prototype.slice.call(root.querySelectorAll(selector));
  }

  // The box that a page puts around a table, which a widget shows and hides with it.
  function boxOf(grid) {
    return grid.closest(".table-wrap") || grid;
  }

  function remember(key, value) {
    try {
      localStorage.setItem(key, value);
    } catch (e) {}
  }

  function recall(key) {
    try {
      return localStorage.getItem(key);
    } catch (e) {
      return null;
    }
  }

  // A table that can be filtered with a few words and sorted by a click on a heading.
  // The filter is kept in the address, as `?filter-ID=words`.
  function table(root, id) {
    var grid = root.querySelector("table");
    if (!grid || !grid.tBodies.length) return false;
    var body = grid.tBodies[0];
    var rows = Array.prototype.slice.call(body.rows);
    var bar = make("div", "dkt-tablebar");
    var input = make("input");
    input.type = "search";
    input.placeholder = "Filter";
    input.setAttribute("aria-label", "Filter the rows");
    var count = make("span", "dkt-count");
    count.setAttribute("role", "status");
    bar.appendChild(input);
    bar.appendChild(count);
    root.insertBefore(bar, root.firstChild);
    var param = "filter-" + id;
    var asked = new URLSearchParams(location.search).get(param);
    if (asked) input.value = asked;

    function refresh() {
      var words = input.value.toLowerCase().split(/\s+/).filter(Boolean);
      var shown = 0;
      rows.forEach(function (row) {
        var text = row.textContent.toLowerCase();
        var keep = words.every(function (word) {
          return text.indexOf(word) >= 0;
        });
        row.hidden = !keep;
        if (keep) shown++;
      });
      count.textContent = shown + " of " + rows.length;
      var params = new URLSearchParams(location.search);
      if (input.value) params.set(param, input.value);
      else params.delete(param);
      var query = params.toString();
      history.replaceState(null, "", location.pathname + (query ? "?" + query : "") + location.hash);
    }
    input.addEventListener("input", refresh);

    var heads = all(grid, "thead th");
    var current = { column: -1, down: false };
    heads.forEach(function (head, column) {
      var sorter = make("button", "dkt-sort");
      sorter.type = "button";
      while (head.firstChild) sorter.appendChild(head.firstChild);
      head.appendChild(sorter);
      sorter.addEventListener("click", function () {
        current.down = current.column === column ? !current.down : false;
        current.column = column;
        rows.sort(function (a, b) {
          var order = (a.cells[column] ? a.cells[column].textContent : "").localeCompare(
            b.cells[column] ? b.cells[column].textContent : "",
            undefined,
            { numeric: true, sensitivity: "base" }
          );
          return current.down ? -order : order;
        });
        rows.forEach(function (row) {
          body.appendChild(row);
        });
        heads.forEach(function (other, k) {
          other.setAttribute("aria-sort", k === column ? (current.down ? "descending" : "ascending") : "none");
        });
      });
    });
    refresh();
    return true;
  }

  // Two pictures, one over the other, with a slider that shows more of the first or of the second.
  function compare(root, id) {
    var images = all(root, "img");
    if (images.length < 2) return false;
    var caption = function (image) {
      var figure = image.closest("figure");
      var text = figure && figure.querySelector("figcaption");
      return (text ? text.textContent : image.alt) || "";
    };
    var names = [caption(images[0]), caption(images[1])];
    var pictures = [images[0], images[1]].map(function (image) {
      var picture = image.closest("picture") || image;
      var figure = picture.closest("figure");
      if (figure) figure.parentNode.removeChild(figure);
      return picture;
    });
    var stage = make("div", "dkt-stage");
    var over = make("div", "dkt-over");
    stage.appendChild(pictures[1]);
    over.appendChild(pictures[0]);
    stage.appendChild(over);
    var slider = make("input");
    slider.type = "range";
    slider.min = "0";
    slider.max = "100";
    slider.value = "50";
    slider.setAttribute("aria-label", "Show more of " + (names[0] || "the first picture") + " or of " + (names[1] || "the second"));
    var handle = make("span", "dkt-handle");
    var tags = [make("span", "dkt-tag dkt-tag-left", names[0]), make("span", "dkt-tag dkt-tag-right", names[1])];
    function move() {
      stage.style.setProperty("--dkt-at", slider.value + "%");
    }
    slider.addEventListener("input", move);
    stage.appendChild(slider);
    stage.appendChild(handle);
    tags.forEach(function (tag) {
      if (tag.textContent) stage.appendChild(tag);
    });
    root.appendChild(stage);
    move();
    return true;
  }

  // A diff, as lines that keep their colours, or as the 2 sides next to each other.
  function diff(root, id) {
    var code = root.querySelector("pre code");
    if (!code) return false;
    var pre = code.parentNode;
    var lines = code.textContent.replace(/\n$/, "").split("\n");
    var kind = function (line) {
      if (/^(diff |index |--- |\+\+\+ )/.test(line)) return "meta";
      if (line.charAt(0) === "@") return "hunk";
      if (line.charAt(0) === "+") return "add";
      if (line.charAt(0) === "-") return "del";
      return "ctx";
    };
    function row(className, text) {
      var line = make("div", "dkt-line " + className);
      line.textContent = text || " ";
      return line;
    }
    var unified = make("div", "dkt-diff");
    lines.forEach(function (line) {
      unified.appendChild(row(kind(line), line));
    });
    // In 2 sides, a run of removals meets the run of additions that follows it.
    var split = make("div", "dkt-diff dkt-split");
    var removed = [];
    var added = [];
    function flush() {
      for (var k = 0; k < Math.max(removed.length, added.length); k++) {
        var pair = make("div", "dkt-pair");
        pair.appendChild(row(removed[k] === undefined ? "gap" : "del", removed[k]));
        pair.appendChild(row(added[k] === undefined ? "gap" : "add", added[k]));
        split.appendChild(pair);
      }
      removed = [];
      added = [];
    }
    lines.forEach(function (line) {
      var k = kind(line);
      if (k === "del") removed.push(line);
      else if (k === "add") added.push(line);
      else {
        flush();
        if (k === "ctx") {
          var pair = make("div", "dkt-pair");
          pair.appendChild(row("ctx", line));
          pair.appendChild(row("ctx", line));
          split.appendChild(pair);
        } else {
          split.appendChild(row(k + " dkt-wide", line));
        }
      }
    });
    flush();
    var views = { Unified: unified, "Side by side": split };
    var switcher = make("div", "dkt-switch");
    var buttons = Object.keys(views).map(function (name) {
      var button = make("button", "dkt-button", name);
      button.type = "button";
      button.addEventListener("click", function () {
        show(name);
        remember("dkt:diff-view", name);
      });
      switcher.appendChild(button);
      return button;
    });
    function show(name) {
      Object.keys(views).forEach(function (other, k) {
        views[other].hidden = other !== name;
        buttons[k].setAttribute("aria-pressed", other === name ? "true" : "false");
      });
    }
    pre.parentNode.insertBefore(switcher, pre);
    pre.parentNode.insertBefore(unified, pre);
    pre.parentNode.insertBefore(split, pre);
    pre.hidden = true;
    show(recall("dkt:diff-view") === "Side by side" ? "Side by side" : "Unified");
    return true;
  }

  var PALETTE = ["#4e79a7", "#f28e2b", "#59a14f", "#e15759", "#b07aa1", "#76b7b2", "#edc948", "#9c755f"];
  var SVG = "http://www.w3.org/2000/svg";

  function node(name, attributes, text) {
    var element = document.createElementNS(SVG, name);
    Object.keys(attributes || {}).forEach(function (key) {
      element.setAttribute(key, attributes[key]);
    });
    if (text !== undefined) element.textContent = text;
    return element;
  }

  // The highest value of a chart, rounded up to a figure that the axis can name.
  function niceMax(value) {
    if (value <= 0) return 1;
    var power = Math.pow(10, Math.floor(Math.log(value) / Math.LN10));
    var steps = [1, 2, 2.5, 5, 10];
    for (var k = 0; k < steps.length; k++) if (steps[k] * power >= value) return steps[k] * power;
    return 10 * power;
  }

  // A table drawn as bars or as lines, with a button to go back to the table.
  function chart(root, id) {
    var grid = root.querySelector("table");
    if (!grid || !grid.tBodies.length) return false;
    var heads = all(grid, "thead th").map(function (cell) {
      return cell.textContent.trim();
    });
    var rows = all(grid.tBodies[0], "tr").map(function (row) {
      return all(row, "td, th").map(function (cell) {
        return cell.textContent.trim();
      });
    });
    var series = heads.slice(1).map(function (name, k) {
      return {
        name: name,
        values: rows.map(function (row) {
          var number = parseFloat((row[k + 1] || "").replace(/[^0-9.\-]/g, ""));
          return isNaN(number) ? 0 : number;
        }),
      };
    });
    if (!series.length || !rows.length) return false;
    var labels = rows.map(function (row) {
      return row[0];
    });
    var line = root.getAttribute("data-type") === "line";
    var width = 640;
    var height = 320;
    var left = 52;
    var bottom = 44;
    var top = 14;
    var right = 14;
    var most = niceMax(
      Math.max.apply(
        null,
        series.map(function (s) {
          return Math.max.apply(null, s.values);
        })
      )
    );
    var plot = { w: width - left - right, h: height - top - bottom };
    var y = function (value) {
      return top + plot.h - (value / most) * plot.h;
    };
    var svg = node("svg", {
      viewBox: "0 0 " + width + " " + height,
      role: "img",
      "aria-label": "Chart of the table",
      "font-size": "12",
      fill: "currentColor",
    });
    for (var tick = 0; tick <= 5; tick++) {
      var value = (most * tick) / 5;
      svg.appendChild(node("line", { x1: left, x2: width - right, y1: y(value), y2: y(value), stroke: "currentColor", "stroke-opacity": ".15" }));
      svg.appendChild(node("text", { x: left - 6, y: y(value) + 4, "text-anchor": "end", "fill-opacity": ".7" }, String(Math.round(value * 100) / 100)));
    }
    var slot = plot.w / labels.length;
    labels.forEach(function (label, k) {
      svg.appendChild(node("text", { x: left + slot * (k + 0.5), y: height - bottom + 18, "text-anchor": "middle", "fill-opacity": ".8" }, label));
    });
    series.forEach(function (s, n) {
      var color = PALETTE[n % PALETTE.length];
      if (line) {
        var points = s.values.map(function (v, k) {
          return left + slot * (k + 0.5) + "," + y(v);
        });
        svg.appendChild(node("polyline", { points: points.join(" "), fill: "none", stroke: color, "stroke-width": "2.2", "stroke-linejoin": "round" }));
        s.values.forEach(function (v, k) {
          var dot = node("circle", { cx: left + slot * (k + 0.5), cy: y(v), r: "3.6", fill: color });
          dot.appendChild(node("title", {}, s.name + ", " + labels[k] + ": " + v));
          svg.appendChild(dot);
        });
      } else {
        var barWidth = (slot * 0.8) / series.length;
        s.values.forEach(function (v, k) {
          var bar = node("rect", {
            x: left + slot * k + slot * 0.1 + barWidth * n,
            y: y(v),
            width: barWidth - 2,
            height: Math.max(0, top + plot.h - y(v)),
            rx: "2",
            fill: color,
          });
          bar.appendChild(node("title", {}, s.name + ", " + labels[k] + ": " + v));
          svg.appendChild(bar);
        });
      }
    });
    var legend = make("p", "dkt-legend");
    series.forEach(function (s, n) {
      var item = make("span", "dkt-key");
      var swatch = make("i");
      swatch.style.background = PALETTE[n % PALETTE.length];
      item.appendChild(swatch);
      item.appendChild(document.createTextNode(s.name));
      legend.appendChild(item);
    });
    var view = make("div", "dkt-chart");
    view.appendChild(svg);
    view.appendChild(legend);
    var switcher = make("div", "dkt-switch");
    var buttons = ["Chart", "Table"].map(function (name) {
      var button = make("button", "dkt-button", name);
      button.type = "button";
      button.addEventListener("click", function () {
        show(name);
      });
      switcher.appendChild(button);
      return button;
    });
    function show(name) {
      view.hidden = name !== "Chart";
      boxOf(grid).hidden = name !== "Table";
      buttons[0].setAttribute("aria-pressed", name === "Chart" ? "true" : "false");
      buttons[1].setAttribute("aria-pressed", name === "Table" ? "true" : "false");
    }
    var anchor = boxOf(grid);
    anchor.parentNode.insertBefore(switcher, anchor);
    anchor.parentNode.insertBefore(view, anchor);
    show("Chart");
    return true;
  }

  // Cards with a front and a back. What the reader knows is kept, and the cards that remain come back.
  function flashcards(root, id) {
    var grid = root.querySelector("table");
    if (!grid || !grid.tBodies.length) return false;
    var cards = all(grid.tBodies[0], "tr").map(function (row) {
      return { front: row.cells[0].innerHTML, back: row.cells[1] ? row.cells[1].innerHTML : "" };
    });
    if (!cards.length) return false;
    var known = dkt.load(id) || [];
    var order = cards.map(function (card, k) {
      return k;
    });
    var flipped = false;
    var face = make("div", "dkt-card");
    face.tabIndex = 0;
    face.setAttribute("role", "button");
    face.setAttribute("aria-label", "Turn the card");
    var bar = dkt.progress();
    var actions = make("div", "dkt-actions");
    var again = dkt.button("Again", function () {
      move(false);
    });
    var got = dkt.button("I know it", function () {
      move(true);
    });
    var reset = dkt.button("Start again", function () {
      known = [];
      dkt.save(id, known);
      order = cards.map(function (card, k) {
        return k;
      });
      flipped = false;
      draw();
    });
    actions.appendChild(again);
    actions.appendChild(got);
    actions.appendChild(reset);
    function remaining() {
      return order.filter(function (k) {
        return known.indexOf(k) < 0;
      });
    }
    function draw() {
      var left = remaining();
      bar.set(cards.length - left.length, cards.length);
      var done = !left.length;
      face.hidden = done;
      again.hidden = got.hidden = done;
      reset.hidden = !done && known.length === 0;
      if (done) return;
      var card = cards[left[0]];
      face.innerHTML = flipped ? card.back : card.front;
      face.setAttribute("data-side", flipped ? "back" : "front");
    }
    function move(knew) {
      var current = remaining()[0];
      if (knew) {
        known.push(current);
        dkt.save(id, known);
      } else {
        order.splice(order.indexOf(current), 1);
        order.push(current);
      }
      flipped = false;
      draw();
    }
    function flip() {
      flipped = !flipped;
      draw();
    }
    face.addEventListener("click", flip);
    face.addEventListener("keydown", function (event) {
      if (event.key === " " || event.key === "Enter") {
        event.preventDefault();
        flip();
      }
    });
    boxOf(grid).hidden = true;
    root.appendChild(bar.node);
    root.appendChild(face);
    root.appendChild(actions);
    draw();
    return true;
  }

  // Numbered pins on a picture, tied to the items of the list that explains them.
  // The pins are positions in percent of the picture, one for each item, in the order of the list.
  function annotated(root, id) {
    var image = root.querySelector("img");
    var list = root.querySelector("ol, ul");
    if (!image || !list) return false;
    var items = Array.prototype.filter.call(list.children, function (item) {
      return item.tagName === "LI";
    });
    var pins = (root.getAttribute("data-pins") || "")
      .split(/\s+/)
      .filter(Boolean)
      .map(function (pin) {
        return pin.split(",").map(Number);
      });
    if (!pins.length || pins.length !== items.length) return false;
    var holder = image.closest("picture") || image;
    var stage = make("div", "dkt-pinned");
    holder.parentNode.insertBefore(stage, holder);
    stage.appendChild(holder);
    // A picture is shown as wide as the page allows, and not wider than it is, unless it is drawn
    // with lines, which has no size of its own.
    function fit() {
      if (!/\.svg(\?|$)/i.test(image.currentSrc || image.src) && image.naturalWidth) {
        stage.style.maxWidth = image.naturalWidth + "px";
      }
    }
    if (image.complete) fit();
    else image.addEventListener("load", fit);
    var marks = pins.map(function (at, k) {
      var mark = make("button", "dkt-pin", String(k + 1));
      mark.type = "button";
      mark.style.left = at[0] + "%";
      mark.style.top = at[1] + "%";
      mark.setAttribute("aria-label", "Note " + (k + 1) + ": " + items[k].textContent.trim().slice(0, 80));
      stage.appendChild(mark);
      return mark;
    });
    function light(k, on) {
      marks[k].classList.toggle("dkt-lit", on);
      items[k].classList.toggle("dkt-lit", on);
    }
    items.forEach(function (item, k) {
      item.tabIndex = -1;
      item.addEventListener("mouseenter", function () {
        light(k, true);
      });
      item.addEventListener("mouseleave", function () {
        light(k, false);
      });
      marks[k].addEventListener("mouseenter", function () {
        light(k, true);
      });
      marks[k].addEventListener("mouseleave", function () {
        light(k, false);
      });
      marks[k].addEventListener("focus", function () {
        light(k, true);
      });
      marks[k].addEventListener("blur", function () {
        light(k, false);
      });
      marks[k].addEventListener("click", function () {
        item.scrollIntoView({ block: "nearest" });
        item.focus({ preventScroll: true });
      });
    });
    return true;
  }

  // A form that writes a command or a file: the table gives the fields, the code block the model.
  // A field with several values is a choice, and a field with one is a text that starts as that value.
  function builder(root, id) {
    var grid = root.querySelector("table");
    var model = root.querySelector("pre code");
    if (!grid || !grid.tBodies.length || !model) return false;
    var fields = all(grid.tBodies[0], "tr").map(function (row) {
      var cells = all(row, "td");
      return {
        name: cells[0].textContent.trim(),
        label: (cells[1] ? cells[1].textContent : cells[0].textContent).trim(),
        values: (cells[2] ? cells[2].textContent : "")
          .split(",")
          .map(function (value) {
            return value.trim();
          })
          .filter(Boolean),
      };
    });
    var template = model.textContent.replace(/\n$/, "");
    var saved = dkt.load(id) || {};
    var current = {};
    var form = make("form", "dkt-form");
    form.addEventListener("submit", function (event) {
      event.preventDefault();
    });
    var out = make("pre");
    var code = make("code");
    out.appendChild(code);
    function write() {
      code.textContent = template.replace(/\{([A-Za-z0-9_-]+)\}/g, function (whole, name) {
        return current[name] !== undefined ? current[name] : whole;
      });
      dkt.save(id, current);
    }
    fields.forEach(function (field) {
      var label = make("label", "dkt-field");
      label.appendChild(make("span", "", field.label));
      var control;
      if (field.values.length > 1) {
        control = make("select");
        field.values.forEach(function (value) {
          var option = make("option", "", value);
          option.value = value;
          control.appendChild(option);
        });
      } else {
        control = make("input");
        control.type = "text";
        control.value = field.values[0] || "";
      }
      if (saved[field.name] !== undefined) control.value = saved[field.name];
      current[field.name] = control.value;
      control.addEventListener("input", function () {
        current[field.name] = control.value;
        write();
      });
      label.appendChild(control);
      form.appendChild(label);
    });
    var box = grid.closest(".table-wrap") || grid;
    box.hidden = true;
    model.parentNode.hidden = true;
    root.appendChild(form);
    root.appendChild(out);
    write();
    return true;
  }

  // A terminal session played back: the lines that start with `$ ` are typed, the others appear.
  function replay(root, id) {
    var model = root.querySelector("pre code");
    if (!model) return false;
    var script = model.textContent.replace(/\n$/, "").split("\n");
    var speed = parseFloat(root.getAttribute("data-speed")) || 1;
    var reduced = window.matchMedia && matchMedia("(prefers-reduced-motion: reduce)").matches;
    var screen = make("pre", "dkt-screen");
    screen.setAttribute("aria-label", "A terminal session");
    var text = make("code");
    var cursor = make("span", "dkt-cursor", "\u258c");
    screen.appendChild(text);
    screen.appendChild(cursor);
    var timer = null;
    var at = 0;
    var typing = 0;
    var running = false;
    var finished = false;
    var play = dkt.button("Play", function () {
      if (finished) restart();
      else toggle();
    });
    var again = dkt.button("Restart", function () {
      restart();
    });
    var bar = make("div", "dkt-controls");
    bar.appendChild(play);
    bar.appendChild(again);
    function label() {
      play.textContent = finished ? "Replay" : running ? "Pause" : "Play";
    }
    function show(all) {
      text.textContent = script.slice(0, at).join("\n") + (typing ? (at ? "\n" : "") + script[at].slice(0, typing) : "");
      if (all) text.textContent = script.join("\n");
    }
    function stop() {
      running = false;
      clearTimeout(timer);
      label();
    }
    function step() {
      if (!running) return;
      if (at >= script.length) {
        finished = true;
        stop();
        return;
      }
      var line = script[at];
      if (line.indexOf("$ ") === 0 && typing < line.length) {
        typing++;
        show();
        timer = setTimeout(step, (typing > 2 ? 45 : 400) / speed);
        return;
      }
      typing = 0;
      at++;
      show();
      timer = setTimeout(step, (line.indexOf("$ ") === 0 ? 500 : 110) / speed);
    }
    function toggle() {
      if (running) return stop();
      running = true;
      label();
      step();
    }
    function restart() {
      stop();
      at = 0;
      typing = 0;
      finished = false;
      text.textContent = "";
      toggle();
    }
    model.parentNode.hidden = true;
    root.appendChild(screen);
    root.appendChild(bar);
    label();
    if (reduced) {
      at = script.length;
      finished = true;
      show(true);
      label();
    } else if ("IntersectionObserver" in window) {
      var seen = new IntersectionObserver(function (entries) {
        if (entries[0].isIntersecting && !finished && !running && at === 0) {
          seen.disconnect();
          toggle();
        }
      });
      seen.observe(screen);
    }
    return true;
  }

  // The versions that come after the one the reader runs. The headings that start with a version
  // cut the document, and the sections of the versions that the reader has are left out.
  function since(root, id) {
    var version = function (text) {
      var m = /^\[?v?(\d+(?:\.\d+)*)/.exec(text.trim());
      return m ? m[1].split(".").map(Number) : null;
    };
    var headings = all(root, "h1, h2, h3, h4, h5, h6").filter(function (heading) {
      return version(heading.textContent);
    });
    if (!headings.length) return false;
    var level = Math.min.apply(
      null,
      headings.map(function (heading) {
        return +heading.tagName.charAt(1);
      })
    );
    var starts = headings.filter(function (heading) {
      return +heading.tagName.charAt(1) === level;
    });
    var sections = starts.map(function (heading) {
      var nodes = [heading];
      for (var next = heading.nextElementSibling; next && starts.indexOf(next) < 0; next = next.nextElementSibling) {
        if (/^H[1-6]$/.test(next.tagName) && +next.tagName.charAt(1) < level) break;
        nodes.push(next);
      }
      return { number: version(heading.textContent), nodes: nodes, text: heading.textContent.trim() };
    });
    var newer = function (a, b) {
      for (var k = 0; k < Math.max(a.length, b.length); k++) {
        if ((a[k] || 0) !== (b[k] || 0)) return (a[k] || 0) > (b[k] || 0);
      }
      return false;
    };
    var picker = make("label", "dkt-since");
    picker.appendChild(document.createTextNode("I run version "));
    var select = make("select");
    var every = make("option", "", "any: show everything");
    every.value = "";
    select.appendChild(every);
    sections.forEach(function (section) {
      var option = make("option", "", section.number.join("."));
      option.value = section.number.join(".");
      select.appendChild(option);
    });
    picker.appendChild(select);
    var count = make("span", "dkt-count");
    count.setAttribute("role", "status");
    picker.appendChild(count);
    root.insertBefore(picker, root.firstChild);
    function apply() {
      var mine = select.value ? select.value.split(".").map(Number) : null;
      var shown = 0;
      sections.forEach(function (section) {
        var keep = !mine || newer(section.number, mine);
        section.nodes.forEach(function (node) {
          node.hidden = !keep;
        });
        if (keep) shown++;
      });
      count.textContent = mine ? shown + " newer version" + (shown === 1 ? "" : "s") : "";
      dkt.save(id, select.value);
    }
    var saved = dkt.load(id);
    if (typeof saved === "string") select.value = saved;
    select.addEventListener("change", apply);
    apply();
    return true;
  }

  // A card with the title and the description of a page, when the pointer rests on a link to it.
  // The cards come from the index that the search reads, so no page is fetched.
  dkt.startPreviews = function () {
    if (!dkt.script || (window.matchMedia && matchMedia("(pointer: coarse)").matches)) return;
    var base = dkt.script.replace(/widgets\.js(\?.*)?$/, "");
    var root = new URL(base).pathname;
    var index = null;
    var loading = null;
    function entries() {
      if (index) return Promise.resolve(index);
      if (!loading) {
        loading = fetch(base + "search.json")
          .then(function (response) {
            return response.json();
          })
          .then(function (list) {
            index = list;
            return list;
          })
          .catch(function () {
            index = [];
            return index;
          });
      }
      return loading;
    }
    var card = make("div", "dkt-preview");
    card.setAttribute("role", "tooltip");
    card.hidden = true;
    document.body.appendChild(card);
    var timer = null;
    var current = null;
    function hide() {
      clearTimeout(timer);
      card.hidden = true;
      current = null;
    }
    function candidate(link) {
      var target;
      try {
        target = new URL(link.href);
      } catch (e) {
        return false;
      }
      if (target.origin !== location.origin || target.pathname.indexOf(root) !== 0) return false;
      if (target.pathname === location.pathname) return false;
      if (link.hasAttribute("download") || link.closest(".dkt-preview, .top, header, footer, .side")) return false;
      return true;
    }
    function show(link) {
      entries().then(function (list) {
        if (current !== link) return;
        var target = new URL(link.href);
        var page = list.filter(function (entry) {
          return entry.u === target.pathname && entry.l === 1;
        })[0];
        var section = target.hash
          ? list.filter(function (entry) {
              return entry.u === target.pathname + target.hash;
            })[0]
          : null;
        var entry = section || page;
        if (!entry) return;
        card.textContent = "";
        card.appendChild(make("strong", "", entry.t));
        var where = section && page ? page.t : entry.b;
        if (where && where !== entry.t) card.appendChild(make("span", "dkt-where", where));
        var text = entry.d || (page && page.d) || "";
        if (text) card.appendChild(make("p", "", text.length > 220 ? text.slice(0, 217) + "..." : text));
        card.hidden = false;
        var box = link.getBoundingClientRect();
        var width = card.offsetWidth;
        var left = Math.max(8, Math.min(window.scrollX + box.left, window.scrollX + document.documentElement.clientWidth - width - 8));
        var below = box.bottom + card.offsetHeight + 12 < window.innerHeight;
        card.style.left = left + "px";
        card.style.top = (below ? window.scrollY + box.bottom + 6 : window.scrollY + box.top - card.offsetHeight - 6) + "px";
      });
    }
    function over(event) {
      var link = event.target.closest && event.target.closest("a[href]");
      if (!link || link === current || !candidate(link)) return;
      hide();
      current = link;
      timer = setTimeout(function () {
        show(link);
      }, 350);
    }
    document.addEventListener("mouseover", over);
    document.addEventListener("focusin", over);
    document.addEventListener("mouseout", function (event) {
      if (current && !(event.relatedTarget && current.contains(event.relatedTarget))) hide();
    });
    document.addEventListener("focusout", hide);
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape") hide();
    });
    window.addEventListener("scroll", hide, { passive: true });
  };

  // What the reader has done in the pages of the site, from what the browser remembers of each widget.
  // widgets.json says what each page holds, and the state of a widget is read the way it was saved.
  function course(root, id) {
    if (!dkt.script) return false;
    var base = dkt.script.replace(/widgets\.js(\?.*)?$/, "");
    var done = function (kind, state) {
      if (kind === "quiz") return typeof state === "number" ? 1 : 0;
      if (!Array.isArray(state)) return 0;
      if (kind === "flashcards") return state.length;
      return state.filter(Boolean).length;
    };
    fetch(base + "widgets.json")
      .then(function (response) {
        return response.json();
      })
      .then(function (pages) {
        var list = make("div", "dkt-course");
        var sum = 0;
        var total = 0;
        var rows = pages.map(function (page) {
          var seen = 0;
          var all = 0;
          page.w.forEach(function (widget) {
            var state = null;
            try {
              state = JSON.parse(localStorage.getItem("dkt:" + page.u + ":" + widget.i));
            } catch (e) {}
            seen += Math.min(widget.n, done(widget.k, state));
            all += widget.n;
          });
          sum += seen;
          total += all;
          var row = make("div", "dkt-row" + (seen === all ? " dkt-done" : ""));
          var link = make("a", "", page.t);
          link.href = page.u;
          var bar = make("progress");
          bar.max = all;
          bar.value = seen;
          row.appendChild(link);
          row.appendChild(bar);
          row.appendChild(make("small", "", seen + "/" + all));
          return row;
        });
        var overall = dkt.progress();
        overall.set(sum, total);
        root.textContent = "";
        list.appendChild(overall.node);
        rows.forEach(function (row) {
          list.appendChild(row);
        });
        root.appendChild(list);
        root.classList.add("dkt-ready");
      })
      .catch(function () {});
    return false;
  }

  builders.table = table;
  builders.compare = compare;
  builders.diff = diff;
  builders.chart = chart;
  builders.flashcards = flashcards;
  builders.annotated = annotated;
  builders.builder = builder;
  builders.replay = replay;
  builders.since = since;
  builders.progress = course;
})();
