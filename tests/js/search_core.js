// Checks the matching of the search palette: node tests/js/search_core.js
const assert = require("assert");
const path = require("path");
const core = require(path.join(__dirname, "../../themes/docs/static/search.js"));

const rows = core.prepare([
  { t: "dkt site", p: "dkt site", u: "/site/", l: 1, d: "Builds a static site from documents." },
  { t: "Theme", p: "dkt site", u: "/site/#theme", l: 2 },
  { t: "The properties file", p: "dkt site", u: "/site/#the-properties-file", l: 2 },
  { t: "The check", p: "dkt site", u: "/site/#the-check", l: 2, a: "no-lint skip" },
  { t: "Templates", p: "dkt site", u: "/site/#templates", l: 2 },
  { t: "Template", p: "dkt html", u: "/html/#template", l: 2 },
  { t: "Properties", p: "Guide", u: "/g/#properties", l: 2, b: "Settings" },
  { t: "Éléphant et café", p: "Zoo", u: "/zoo/#e", l: 2 },
  { t: "dkt cat", p: "dkt cat", u: "/cat/", l: 1, d: "Renders Markdown on the terminal." },
]);
const top = (q) => core.find(rows, q).map((h) => h.row.t);

assert.strictEqual(core.fold("Éléphant").text, "elephant");
assert.strictEqual(core.fold("Éléphant").map.length, "elephant".length);

// case and accents are ignored
assert.deepStrictEqual(top("ELEPHANT"), ["Éléphant et café"]);
assert.deepStrictEqual(top("cafe"), ["Éléphant et café"]);

// every word is needed, in any order
assert.deepStrictEqual(top("file properties"), ["The properties file"]);
assert.deepStrictEqual(top("theme zebra"), []);

// an exact title ranks above a word start, which ranks above a substring
assert.strictEqual(top("template")[0], "Template");
assert.strictEqual(top("properties")[0], "Properties");
assert.ok(top("emplate").length >= 2);

// the page that names a section counts, below the title
assert.strictEqual(top("site theme")[0], "Theme");

// an alias finds a section that its title does not name
assert.strictEqual(top("no-lint")[0], "The check");
assert.strictEqual(top("skip")[0], "The check");

// the description finds a page, under the titles
assert.strictEqual(top("static")[0], "dkt site");
assert.ok(top("terminal").includes("dkt cat"));

// one slip of the keys still finds a title
assert.ok(top("tempaltes").includes("Templates"));
assert.ok(top("propertis").includes("The properties file"));
assert.deepStrictEqual(top("zzzzz"), []);

// the letters in order find a long title
assert.ok(top("prpfile").includes("The properties file"));

// the scope prefixes
assert.ok(top("#site").every((t) => t !== "dkt site"));
assert.deepStrictEqual(top(">site"), ["dkt site"]);
assert.deepStrictEqual(top(">theme"), []);

// the swap, the missing and the extra letter
assert.ok(core.close("teh", "the"));
assert.ok(core.close("themee", "theme"));
assert.ok(core.close("thme", "theme"));
assert.ok(!core.close("theme", "thyme!"));

// an empty query finds nothing
assert.deepStrictEqual(top("   "), []);
// a title with a parenthesis is matched exactly without it, and ranks above a longer name
const kw = core.prepare([
  { t: "maxconnrate", p: "Guide", u: "/a/#r", l: 2 },
  { t: "maxconn (Global section)", p: "Guide", u: "/a/#g", l: 2 },
  { t: "maxconn", p: "Guide", u: "/a/#ref", l: 2 },
]);
assert.deepStrictEqual(core.find(kw, "maxconn").map((h) => h.row.u), ["/a/#ref", "/a/#g", "/a/#r"]);

// a result under the address of the page being read ranks above the same result elsewhere
const versions = core.prepare([
  { t: "bind", p: "Manual 3.5", u: "/h/3.5/c/#bind", l: 2 },
  { t: "bind", p: "Manual 3.4", u: "/h/3.4/c/#bind", l: 2 },
]);
assert.strictEqual(core.find(versions, "bind")[0].row.u, "/h/3.5/c/#bind");
assert.strictEqual(core.find(versions, "bind", "/h/3.4/")[0].row.u, "/h/3.4/c/#bind");

console.log("search core: ok");
