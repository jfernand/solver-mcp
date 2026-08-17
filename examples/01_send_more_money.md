# SEND + MORE = MONEY

The classic cryptarithmetic puzzle: assign each letter `S E N D M O R Y` a
distinct digit 0-9 so that the sum holds as ordinary base-10 addition,
with no leading zero on `SEND` or `MORE` (so `S != 0` and `M != 0`):

```
  S E N D
+ M O R E
---------
M O N E Y
```

## Formulation

- One `int_range` variable per letter. `S` and `M` get domain `1..9`
  (leading digits can't be zero); the rest get `0..9`.
- `all_different` over all eight letters.
- The arithmetic itself needs no `times`/`plus` decomposition -- it's a
  single weighted sum. Expanding
  `1000S + 100E + 10N + D + 1000M + 100O + 10R + E - (10000M + 1000O + 100N + 10E + Y) = 0`
  and collecting each letter's coefficient gives:

  | letter | S | E | N | D | M | O | R | Y |
  |---|---|---|---|---|---|---|---|---|
  | coefficient | 1000 | 91 | -90 | 1 | -9000 | -900 | 10 | -1 |

  (`E`'s coefficient is `100 - 10 = 90`... plus the `+1E` from `MORE`,
  i.e. `100 + 1 - 10 = 91`; `N`'s is `10 - 100 = -90`; `M`'s is
  `1000 - 10000 = -9000`; `O`'s is `100 - 1000 = -900`.) One
  `linear_eq` posts that sum against `rhs: 0`.

The known solution `SEND=9567, MORE=1085, MONEY=10652` satisfies this
(`1000*9 + 91*5 - 90*6 + 1*7 - 9000*1 - 900*0 + 10*8 - 1*2 = 0`), so it's
a sanity check for the coefficients above, not something the model is
told directly.

```json
{
  "variables": [
    { "kind": "int_range", "name": "S", "min": 1, "max": 9 },
    { "kind": "int_range", "name": "E", "min": 0, "max": 9 },
    { "kind": "int_range", "name": "N", "min": 0, "max": 9 },
    { "kind": "int_range", "name": "D", "min": 0, "max": 9 },
    { "kind": "int_range", "name": "M", "min": 1, "max": 9 },
    { "kind": "int_range", "name": "O", "min": 0, "max": 9 },
    { "kind": "int_range", "name": "R", "min": 0, "max": 9 },
    { "kind": "int_range", "name": "Y", "min": 0, "max": 9 }
  ],
  "constraints": [
    {
      "kind": "all_different",
      "vars": [
        { "var": "S" }, { "var": "E" }, { "var": "N" }, { "var": "D" },
        { "var": "M" }, { "var": "O" }, { "var": "R" }, { "var": "Y" }
      ]
    },
    {
      "kind": "linear_eq",
      "terms": [
        { "var": "S", "scale": 1000 },
        { "var": "E", "scale": 91 },
        { "var": "N", "scale": -90 },
        { "var": "D", "scale": 1 },
        { "var": "M", "scale": -9000 },
        { "var": "O", "scale": -900 },
        { "var": "R", "scale": 10 },
        { "var": "Y", "scale": -1 }
      ],
      "rhs": 0
    }
  ],
  "solve": { "mode": "satisfy" },
  "max_time_seconds": 10
}
```
