# Japanese grammar data

The five JSON files in `openjlpt/` are unmodified grammar data from the
[OpenJLPT contributors](https://github.com/evanclan/OpenJLPT/tree/0d1d3410bec90bd4098a7c72de820543cb4f707c/data/json/grammar),
licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).
See [LICENSE](openjlpt/LICENSE) and [NOTICE](openjlpt/NOTICE.md) for attribution.
The levels are community estimates.

`zh-Hans.json` contains original Chinese explanations, formation notes and examples
written for Atogaki, plus 21 foundation concept entries. Entries mapped to the catalog
retain the OpenJLPT IDs.

`zh-Hans-openjlpt.json` is the Simplified Chinese translation of the remaining
OpenJLPT entries, prepared with local Hy-MT2 and assembled by stable ID. It
contains translations of formation, meaning, usage and the upstream examples,
in the original example order. Both overlays are published under CC BY-SA 4.0.
The parser source code is distributed under the project's Apache-2.0 license.
