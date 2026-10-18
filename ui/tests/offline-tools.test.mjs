import test from "node:test";
import assert from "node:assert/strict";
import { matchesQuery, highlightSaved, candidateTextDiff } from "../src/offline-tools.ts";
test("search covers normalized Japanese, English casing and empty queries", () => {
  assert.equal(matchesQuery("ＡＢＣ", [null, "abc podcast"]), true);
  assert.equal(matchesQuery("庭", ["湖吉の庭"]), true);
  assert.equal(matchesQuery("missing", ["湖吉の庭"]), false);
  assert.equal(matchesQuery("   ", []), true);
});
test("saved highlights preserve UTF-16 text and escape markup", () => {
  assert.equal(
    highlightSaved("😀勉強<&", ["勉強"], [[2, 4]]),
    '😀<mark class="saved-expression">勉強</mark>&lt;&amp;',
  );
  assert.equal(
    highlightSaved("abcabc", ["abc", "bc"]),
    ' <mark class="saved-expression">abcabc</mark>'.trim(),
  );
  assert.equal(
    highlightSaved(
      "abc",
      [],
      [
        [0, 99],
        [-1, 2],
      ],
    ),
    "abc",
  );
});

test("English saved words do not highlight inside a different word", () => {
  assert.equal(
    highlightSaved("in things in", ["in"], [], "en"),
    '<mark class="saved-expression">in</mark> things <mark class="saved-expression">in</mark>',
  );
});

test('candidate diff preserves common Unicode text and escapes changes',()=>{
  assert.equal(candidateTextDiff('😀a<','😀b<'),'😀<del>a</del><ins>b</ins>&lt;');
  assert.equal(candidateTextDiff('<same>','<same>'),'&lt;same&gt;');
});
