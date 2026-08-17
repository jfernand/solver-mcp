# Dinner party logic

Deciding who attends a dinner party -- Alice, Bob, Carol, Dana -- under
social rules:

1. At least one of Alice or Bob attends.
2. If Carol attends, Dana attends too.
3. Dana is separately considering the after-party; if she commits to
   it (`dana_after_party`), she must be at dinner too.

List every valid combination of attendance (and the after-party
decision). By hand-enumeration there are 15: for each of the 9 ways to
satisfy rules 1 and 2, `dana_after_party` can be `true` only when
`attends_dana` is already `true` (giving it 2 free choices for the 6
combinations where Dana attends, and forcing it to `false` for the 3
where she doesn't) -- `6*2 + 3*1 = 15`.

## Formulation

- `attends_alice`, `attends_bob`, `attends_carol`, `attends_dana`,
  `dana_after_party`: all `bool`.
- Rule 1 is a direct `clause`: `attends_alice \/ attends_bob`.
- Rule 2, "Carol implies Dana", is the standard implication-as-clause
  encoding: `not(Carol) \/ Dana`, i.e. `clause` with Carol's literal
  negated.
- Rule 3 is genuinely conditional on a *variable*, not a fixed clause,
  so it's `linear_eq(attends_dana == 1)` posted with
  `reification: implied_by(dana_after_party)` -- "if she commits to the
  after-party, then she attends dinner" -- rather than folded into a
  `clause`. This is the constraint kind `implied_by` is for: attaching
  a numeric/global constraint to a boolean condition, which `clause`
  alone can't express directly.
- As an extra (non-essential) derived fact, `full_house` is fully
  reified (`reify`, the `<->` form) against `conjunction([attends_alice,
  attends_bob, attends_carol, attends_dana])` -- true exactly when
  everyone attends. It doesn't add or remove any solutions; each of the
  15 valid attendance patterns determines its own value for it.
- `solve.mode: enumerate` lists every valid combination instead of
  stopping at the first one.

```json
{
  "variables": [
    { "kind": "bool", "name": "attends_alice" },
    { "kind": "bool", "name": "attends_bob" },
    { "kind": "bool", "name": "attends_carol" },
    { "kind": "bool", "name": "attends_dana" },
    { "kind": "bool", "name": "dana_after_party" },
    { "kind": "bool", "name": "full_house" }
  ],
  "constraints": [
    {
      "kind": "clause",
      "literals": [{ "var": "attends_alice" }, { "var": "attends_bob" }]
    },
    {
      "kind": "clause",
      "literals": [{ "var": "attends_carol", "negated": true }, { "var": "attends_dana" }]
    },
    {
      "kind": "linear_eq",
      "terms": [{ "var": "attends_dana" }],
      "rhs": 1,
      "reification": { "mode": "implied_by", "literal": { "var": "dana_after_party" } }
    },
    {
      "kind": "conjunction",
      "literals": [
        { "var": "attends_alice" }, { "var": "attends_bob" },
        { "var": "attends_carol" }, { "var": "attends_dana" }
      ],
      "reification": { "mode": "reify", "literal": { "var": "full_house" } }
    }
  ],
  "solve": { "mode": "enumerate", "max_solutions": 20 },
  "max_time_seconds": 5
}
```
