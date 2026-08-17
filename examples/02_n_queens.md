# 8-Queens

Place 8 queens on an 8x8 chessboard so that no two attack each other:
no shared row, column, or diagonal.

## Formulation

One variable `queens_i` per row `i` (`0..7`), holding the *column* of
the queen in that row -- so "no shared row" is automatic (one variable
per row), and "no shared column" is `all_different(queens)`.

The diagonal constraints are the interesting part, and they're where
`Expr`'s `offset` earns its keep: two queens on the same
up-diagonal have `column - row` equal; on the same down-diagonal they
have `column + row` equal. So:

- `all_different` over `queens_i + i` for each `i` (down-diagonals)
- `all_different` over `queens_i - i` for each `i` (up-diagonals)

Each is a single `all_different` constraint whose members are
`{"var": "queens_i", "offset": i}` / `{"var": "queens_i", "offset": -i}`
-- no auxiliary variables needed, since `offset` is a free affine view
in Pumpkin, not a posted constraint.

```json
{
  "variables": [
    { "kind": "int_range", "name": "queens_0", "min": 0, "max": 7 },
    { "kind": "int_range", "name": "queens_1", "min": 0, "max": 7 },
    { "kind": "int_range", "name": "queens_2", "min": 0, "max": 7 },
    { "kind": "int_range", "name": "queens_3", "min": 0, "max": 7 },
    { "kind": "int_range", "name": "queens_4", "min": 0, "max": 7 },
    { "kind": "int_range", "name": "queens_5", "min": 0, "max": 7 },
    { "kind": "int_range", "name": "queens_6", "min": 0, "max": 7 },
    { "kind": "int_range", "name": "queens_7", "min": 0, "max": 7 }
  ],
  "constraints": [
    {
      "kind": "all_different",
      "vars": [
        { "var": "queens_0" }, { "var": "queens_1" }, { "var": "queens_2" }, { "var": "queens_3" },
        { "var": "queens_4" }, { "var": "queens_5" }, { "var": "queens_6" }, { "var": "queens_7" }
      ]
    },
    {
      "kind": "all_different",
      "vars": [
        { "var": "queens_0", "offset": 0 },
        { "var": "queens_1", "offset": 1 },
        { "var": "queens_2", "offset": 2 },
        { "var": "queens_3", "offset": 3 },
        { "var": "queens_4", "offset": 4 },
        { "var": "queens_5", "offset": 5 },
        { "var": "queens_6", "offset": 6 },
        { "var": "queens_7", "offset": 7 }
      ]
    },
    {
      "kind": "all_different",
      "vars": [
        { "var": "queens_0", "offset": 0 },
        { "var": "queens_1", "offset": -1 },
        { "var": "queens_2", "offset": -2 },
        { "var": "queens_3", "offset": -3 },
        { "var": "queens_4", "offset": -4 },
        { "var": "queens_5", "offset": -5 },
        { "var": "queens_6", "offset": -6 },
        { "var": "queens_7", "offset": -7 }
      ]
    }
  ],
  "solve": { "mode": "satisfy" },
  "max_time_seconds": 10
}
```
