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

  builders.table = table;
  builders.compare = compare;
  builders.diff = diff;
  builders.chart = chart;
  builders.flashcards = flashcards;
})();
