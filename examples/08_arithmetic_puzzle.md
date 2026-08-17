# Arithmetic puzzle

Find positive integers `a` (1-9), `b` (1-9), and `c` such that:

- `a` times `b` equals `c`
- `b` divided by `a`, using truncating (round-toward-zero) division,
  equals exactly 2
- the absolute difference between `a` and `b` is 3
- `c` is the largest of the three numbers, and `a` is the smallest
- `a` plus `b` is strictly less than 10

By hand: if `b = a + 3` and truncating `b / a = 2`, then
`2a <= b < 3a`, i.e. `2a <= a+3 < 3a`, which forces `a` in `{2, 3}`.
`a=2` gives `b=5, c=10` (out of `c`'s intended range below);
`a=3` gives `b=6, c=18`, and `a+b=9 < 10` holds. So `a=3, b=6, c=18`.

## Formulation

This is one worked example touching every remaining arithmetic
constraint kind at once:

- `times(a, b, c)` -- `a * b = c`.
- `division(numerator: b, denominator: a, rhs: q)` where `q` is pinned
  to `2` via `[2, 2]` domain (truncating division, per Pumpkin's
  semantics -- `a`'s domain must exclude 0, which `1..9` already does).
- `absolute(signed: diff, absolute: three)`, where `diff` is an
  auxiliary variable tied to `a - b` by a `linear_eq`
  (`diff` can't be written directly as a single `Expr`, since `Expr` is
  an affine view of *one* variable, not a combination of two -- this is
  the same pattern `GroupedCspRequest::Distance` already uses in
  `csp_tools.rs`), and `three` is pinned to `3`.
- `maximum([a, b, c], m: c)` / `minimum([a, b, c], m: a)` -- note `c`
  and `a` are each both a member of the array *and* the constrained
  extremum; that's fine, it just means "c happens to be the largest of
  the three, enforce it."
- `plus(a, b, s)` -- a fresh variable `s = a + b` -- followed by
  `linear_lt` to say `s < 10`.

```json
{
  "variables": [
    { "kind": "int_range", "name": "a", "min": 1, "max": 9 },
    { "kind": "int_range", "name": "b", "min": 1, "max": 9 },
    { "kind": "int_range", "name": "c", "min": 1, "max": 20 },
    { "kind": "int_range", "name": "s", "min": 2, "max": 18 },
    { "kind": "int_range", "name": "diff", "min": -8, "max": 8 },
    { "kind": "int_range", "name": "three", "min": 3, "max": 3 },
    { "kind": "int_range", "name": "q", "min": 2, "max": 2 }
  ],
  "constraints": [
    { "kind": "times", "a": { "var": "a" }, "b": { "var": "b" }, "c": { "var": "c" } },
    { "kind": "division", "numerator": { "var": "b" }, "denominator": { "var": "a" }, "rhs": { "var": "q" } },
    { "kind": "linear_eq", "terms": [{ "var": "a" }, { "var": "b", "scale": -1 }, { "var": "diff", "scale": -1 }], "rhs": 0 },
    { "kind": "absolute", "signed": { "var": "diff" }, "absolute": { "var": "three" } },
    { "kind": "maximum", "array": [{ "var": "a" }, { "var": "b" }, { "var": "c" }], "m": { "var": "c" } },
    { "kind": "minimum", "array": [{ "var": "a" }, { "var": "b" }, { "var": "c" }], "m": { "var": "a" } },
    { "kind": "plus", "a": { "var": "a" }, "b": { "var": "b" }, "c": { "var": "s" } },
    { "kind": "linear_lt", "terms": [{ "var": "s" }], "rhs": 10 }
  ],
  "solve": { "mode": "satisfy" },
  "max_time_seconds": 5
}
```
