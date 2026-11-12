---
name: clean-room-writing
description: Use before and after writing or changing any Statscraft help text, documentation page, error or warning message, explanatory code comment, example syntax, or test program and test data, whenever the behaviour was learned from the reference manuals. Keeps the manuals' wording, examples and sample data out of every repository and checks the result.
---

# Clean-room writing

The command language and the .sav format are free to implement. The manuals' text, their example programs and their sample data are not: in SAS Institute v World Programming the clone's language was lawful, but its manual infringed because it reproduced the vendor manual's wording. Everything we publish must be written by us, from our understanding.

## While writing

1. **Read for behaviour, then close the manual.** Note the rules as short facts in your own words (defaults, missing values, weights, edge cases). Write the text from those notes, never from the manual's sentences.
2. **Write it our way.** Our own sentence structure and our own order of points, in the Statscraft voice: short sentences, one idea each, American spelling. Do not paraphrase sentence by sentence; a reworded copy is still a copy.
3. **Invent every example and every data row.** Use our own scenario, variable names, record layouts, codes and numbers, for example the synthetic fixtures in `tests/fixtures/` or a new made-up topic. Never start from a manual example and rename things. An example must not resemble a manual example in topic, layout or values.
4. **Nothing from the manual goes anywhere**: not into code, comments, commit messages, issues, pull requests or notes kept in a repository.

## After writing: check

5. **Run the overlap check** on every file you changed. It lives only on the owner's machine, next to the local manuals, in the workspace folder that holds the repositories:

   ```
   python3 ../lab/tools/manual_overlap.py --files <changed files>     # from the workspace: lab/tools/...
   ```

   - `PROSE`: rewrite the whole passage from your notes, not by swapping words.
   - `SHORT`: rewrite it, unless it is a common technical phrase anyone would use (for example "the harmonic mean of the group sizes").
   - `LONG SYNTAX`: if it is a manual example program or manual data, replace it with your own.

   The pre-commit hook runs the same check on staged files and refuses prose hits.

6. **Look for paraphrase the check cannot see.** It finds only near-verbatim text. For any help entry or documentation section longer than a paragraph, compare your text with the manual section you read. If the order of points, the examples or distinctive phrasing follow the manual, rewrite. When unsure, give both texts to an independent subagent and ask whether a reader would say the second was written by rewording the first; on yes or maybe, rewrite.

Where the check is not available (CI, another machine), steps 1 to 4 and 6 still apply.
