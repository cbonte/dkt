// Checks the choice of the selector of versions: node tests/js/versions_core.js
const assert = require("assert");
const path = require("path");
const core = require(path.join(__dirname, "../../themes/common/static/versions.js"));

// the path of a page inside its version
assert.strictEqual(core.inside("/docs/1.1/guide/a/", "/docs", "1.1"), "guide/a/");
assert.strictEqual(core.inside("/docs/1.1/", "/docs", "1.1"), "");
assert.strictEqual(core.inside("/docs/1.1/guide/a/index.html", "/docs", "1.1"), "guide/a/");
assert.strictEqual(core.inside("/docs/1.10/guide/", "/docs", "1.1"), null);
assert.strictEqual(core.inside("/1.1/x/", "", "1.1"), "x/");

const pages = { "": [], "guide/": [], "guide/a/": ["page-a", "install", "café"] };

// the page and the anchor
assert.deepStrictEqual(core.choose("guide/a/", "#install", "/docs", "1.0", pages), {
  url: "/docs/1.0/guide/a/#install",
  lost: null,
});
// an anchor written with escapes is found by its text
assert.deepStrictEqual(core.choose("guide/a/", "#caf%C3%A9", "/docs", "1.0", pages), {
  url: "/docs/1.0/guide/a/#caf%C3%A9",
  lost: null,
});
// the page without the anchor
assert.deepStrictEqual(core.choose("guide/a/", "#gone", "/docs", "1.0", pages), {
  url: "/docs/1.0/guide/a/",
  lost: "anchor",
});
// the page without an anchor asked
assert.deepStrictEqual(core.choose("guide/a/", "", "/docs", "1.0", pages), {
  url: "/docs/1.0/guide/a/",
  lost: null,
});
// no page: the home of the version
assert.deepStrictEqual(core.choose("guide/z/", "#install", "/docs", "1.0", pages), {
  url: "/docs/1.0/",
  lost: "page",
});
// the home leads to the home, with nothing to tell
assert.deepStrictEqual(core.choose("", "", "/docs", "1.0", {}), { url: "/docs/1.0/", lost: null });
// a page outside the version, or a list that could not be read
assert.deepStrictEqual(core.choose(null, "#x", "/docs", "1.0", pages), { url: "/docs/1.0/", lost: null });
assert.deepStrictEqual(core.choose("guide/a/", "", "/docs", "1.0", undefined), {
  url: "/docs/1.0/",
  lost: "page",
});

// the warning follows the list as it is now
const list = {
  format: 1,
  latest: "1.1",
  versions: [
    { name: "2.0", title: "2.0-dev", status: "dev", url: "/docs/2.0/" },
    { name: "1.1", title: "1.1", status: "release", url: "/docs/1.1/" },
    { name: "1.0", title: "1.0", status: "release", url: "/docs/1.0/" },
    { name: "0.9", title: "0.9", status: "deprecated", url: "/docs/0.9/" },
  ],
};
assert.strictEqual(core.warning(list, "2.0"), "dev");
assert.strictEqual(core.warning(list, "1.1"), null);
assert.strictEqual(core.warning(list, "1.0"), "newer");
assert.strictEqual(core.warning(list, "0.9"), "deprecated");
assert.strictEqual(core.warning(list, "3.0"), undefined);
assert.strictEqual(core.warning({ versions: list.versions.slice(1, 3) }, "1.0"), null);

// the state of a version is written as text
assert.strictEqual(core.label(list.versions[0], "1.1"), "2.0-dev (development)");
assert.strictEqual(core.label(list.versions[1], "1.1"), "1.1 (latest)");
assert.strictEqual(core.label(list.versions[2], "1.1"), "1.0");
assert.strictEqual(core.label(list.versions[3], "1.1"), "0.9 (deprecated)");
assert.strictEqual(
  core.sentence("newer", "1.0"),
  "This is the documentation of 1.0, and a newer version exists."
);
console.log("ok");
