# Mini Sudoku (4x4)

Fill a 4x4 grid with digits 1-4 so that every row, every column, and
every 2x2 box contains each digit exactly once. Given clues (row,
column, using 0-based indices from the top-left):

```
1 . . 4
. . 1 .
. 1 . .
4 . . 1
```

## Formulation

One `int_range` variable `cell_r_c` per grid position. Clue cells are
pinned by giving them domain `[v, v]` directly at declaration time
(the same "constant variable" trick used for distances in the TSP
example) rather than adding separate equality constraints; free cells
get the full `1..4` domain. Then it's twelve `all_different`
constraints: four rows, four columns, four 2x2 boxes.

```json
{
  "variables": [
    { "kind": "int_range", "name": "cell_0_0", "min": 1, "max": 1 },
    { "kind": "int_range", "name": "cell_0_1", "min": 1, "max": 4 },
    { "kind": "int_range", "name": "cell_0_2", "min": 1, "max": 4 },
    { "kind": "int_range", "name": "cell_0_3", "min": 4, "max": 4 },

    { "kind": "int_range", "name": "cell_1_0", "min": 1, "max": 4 },
    { "kind": "int_range", "name": "cell_1_1", "min": 1, "max": 4 },
    { "kind": "int_range", "name": "cell_1_2", "min": 1, "max": 1 },
    { "kind": "int_range", "name": "cell_1_3", "min": 1, "max": 4 },

    { "kind": "int_range", "name": "cell_2_0", "min": 1, "max": 4 },
    { "kind": "int_range", "name": "cell_2_1", "min": 1, "max": 1 },
    { "kind": "int_range", "name": "cell_2_2", "min": 1, "max": 4 },
    { "kind": "int_range", "name": "cell_2_3", "min": 1, "max": 4 },

    { "kind": "int_range", "name": "cell_3_0", "min": 4, "max": 4 },
    { "kind": "int_range", "name": "cell_3_1", "min": 1, "max": 4 },
    { "kind": "int_range", "name": "cell_3_2", "min": 1, "max": 4 },
    { "kind": "int_range", "name": "cell_3_3", "min": 1, "max": 1 }
  ],
  "constraints": [
    { "kind": "all_different", "vars": [{ "var": "cell_0_0" }, { "var": "cell_0_1" }, { "var": "cell_0_2" }, { "var": "cell_0_3" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_1_0" }, { "var": "cell_1_1" }, { "var": "cell_1_2" }, { "var": "cell_1_3" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_2_0" }, { "var": "cell_2_1" }, { "var": "cell_2_2" }, { "var": "cell_2_3" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_3_0" }, { "var": "cell_3_1" }, { "var": "cell_3_2" }, { "var": "cell_3_3" }] },

    { "kind": "all_different", "vars": [{ "var": "cell_0_0" }, { "var": "cell_1_0" }, { "var": "cell_2_0" }, { "var": "cell_3_0" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_0_1" }, { "var": "cell_1_1" }, { "var": "cell_2_1" }, { "var": "cell_3_1" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_0_2" }, { "var": "cell_1_2" }, { "var": "cell_2_2" }, { "var": "cell_3_2" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_0_3" }, { "var": "cell_1_3" }, { "var": "cell_2_3" }, { "var": "cell_3_3" }] },

    { "kind": "all_different", "vars": [{ "var": "cell_0_0" }, { "var": "cell_0_1" }, { "var": "cell_1_0" }, { "var": "cell_1_1" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_0_2" }, { "var": "cell_0_3" }, { "var": "cell_1_2" }, { "var": "cell_1_3" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_2_0" }, { "var": "cell_2_1" }, { "var": "cell_3_0" }, { "var": "cell_3_1" }] },
    { "kind": "all_different", "vars": [{ "var": "cell_2_2" }, { "var": "cell_2_3" }, { "var": "cell_3_2" }, { "var": "cell_3_3" }] }
  ],
  "solve": { "mode": "satisfy" },
  "max_time_seconds": 10
}
```
