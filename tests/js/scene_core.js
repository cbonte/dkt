// Checks the logic of the scenes: node tests/js/scene_core.js
const assert = require("assert");
const path = require("path");
const core = require(path.join(__dirname, "../../themes/common/static/widgets-extra.js"));

// a step turns off before it lights, and the parts lit stay lit
const states = core.sceneStates(["gate", "core wire", "-gate", "-* gate", "gate -gate", "", null]);
assert.deepStrictEqual(
  states.map((s) => s.lit),
  [["gate"], ["core", "gate", "wire"], ["core", "wire"], ["gate"], ["gate"], ["gate"], ["gate"]]
);
assert.deepStrictEqual(
  states.map((s) => s.fresh),
  [["gate"], ["core", "wire"], [], ["gate"], ["gate"], [], []]
);
// a part named like a property of an object is a part as any other
assert.deepStrictEqual(core.sceneStates(["__proto__ constructor"])[0].lit, ["__proto__", "constructor"]);

// a step stays the time its text takes to read, and never less than the shortest pace
const short = core.scenePace("A request arrives.");
assert.strictEqual(short, core.scenePace(""));
assert.ok(core.scenePace("word ".repeat(40)) > short);

// the player goes through the steps and stops at the last one
let p = core.scenePlayer(3, false);
assert.deepStrictEqual([p.at, p.playing], [0, false]);
p.autostart();
assert.ok(p.playing);
p.tick();
assert.deepStrictEqual([p.at, p.playing], [1, true]);
p.tick();
assert.deepStrictEqual([p.at, p.playing], [2, false]);
p.tick();
assert.strictEqual(p.at, 2);
// play at the end starts again from the first step
p.play();
assert.deepStrictEqual([p.at, p.playing], [0, true]);
// an action of the reader pauses the scene
p.next();
assert.deepStrictEqual([p.at, p.playing], [1, false]);
p.toggle();
assert.ok(p.playing);
p.toggle();
assert.ok(!p.playing);
p.go(9);
assert.strictEqual(p.at, 2);
p.go(-4);
assert.strictEqual(p.at, 0);
p.previous();
assert.strictEqual(p.at, 0);
// after an action, the automatic start does nothing
p.autostart();
assert.ok(!p.playing);
p.restart();
assert.deepStrictEqual([p.at, p.playing], [0, true]);

// a reader who asks for less motion gets no automatic start, and a restart that does not play
p = core.scenePlayer(3, true);
p.autostart();
assert.ok(!p.playing);
p.go(2);
p.restart();
assert.deepStrictEqual([p.at, p.playing], [0, false]);
// the reader may still play the scene
p.play();
assert.ok(p.playing);

// a scene of 1 step never plays
p = core.scenePlayer(1, false);
p.autostart();
p.play();
assert.ok(!p.playing);
console.log("ok");
