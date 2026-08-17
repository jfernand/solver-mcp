//! A general intermediate representation (IR) for problems solved by
//! Pumpkin, Rust's Lazy Clause Generation CP solver -- as opposed to the
//! narrow, hand-shaped request types in `csp_tools.rs` (`CspRequest`,
//! `GroupedCspRequest`, `ScheduleRequest`), each of which only exercises a
//! sliver of what the solver can actually express.
//!
//! # Design
//!
//! Pumpkin's public surface (`pumpkin_constraints`, v0.5.0) is a flat list
//! of constraint-constructor functions over integer variables and boolean
//! literals -- there is no composable expression tree in the solver itself
//! (nested expressions like `abs(a - b) + c` don't exist as one call; you
//! decompose them into a handful of these primitives wired together with
//! auxiliary variables, which is exactly what `GroupedCspRequest::Distance`
//! already does by hand for `abs`). So the natural IR here mirrors that
//! shape -- closer to FlatZinc than to an AST with nested `Expr` nodes:
//!
//! 1. Declare all variables up front (int or bool), each with a name.
//! 2. List constraints as a flat sequence, one entry per
//!    `pumpkin_constraints` function, referencing variables by name.
//! 3. Say how to solve: first solution, enumerate several, or optimise an
//!    objective variable.
//!
//! Two reference types cover every argument position in every constraint:
//!
//! - [`Expr`] -- `scale * var + offset`, an affine view of an int-or-bool
//!   variable. This exists natively in Pumpkin as `AffineView` /
//!   `TransformableVariable::{scaled, offset}` and costs nothing extra (no
//!   fresh variable, no propagator) -- it's how the existing code encodes
//!   "the green house is one past the ivory house" as `Equals(green,
//!   ivory.scaled(1).offset(1))`-ish today. A bare reference is
//!   `{"var": "x"}` (scale 1, offset 0).
//! - [`BoolRef`] -- a possibly-negated reference to a `Bool` variable,
//!   used only where Pumpkin's type system wants an actual `Literal`
//!   rather than an integer view: clauses, conjunctions, and the
//!   reification handle attached to any constraint.
//!
//! Bool variables are *also* usable inside [`Expr`] (via
//! `Literal::get_integer_variable()`, a 0/1-valued `DomainId`), which is
//! why there's no separate `BooleanLeq`/`BooleanEq` constraint kind here:
//! `pumpkin_constraints::boolean_less_than_or_equals` /
//! `boolean_equals` are themselves just thin wrappers that convert bools
//! to their integer view and delegate to `less_than_or_equals` / `equals`
//! (see `pumpkin-constraints-0.5.0/src/constraints/boolean.rs`). Routing
//! through `LinearLeq` / `LinearEq` with `Expr`s over `Bool` variables
//! covers the same ground with less surface area, and additionally allows
//! mixing bools and ints in one linear constraint, which the dedicated
//! functions don't.
//!
//! # Reification
//!
//! Every constraint in Pumpkin can be posted three ways:
//!
//! - **Always true**: `.post()` -- no `reification` field.
//! - **Half-reified** (`r -> constraint`): `.implied_by(r)` -- available
//!   for *every* constraint kind. `Reification::ImpliedBy`.
//! - **Fully reified** (`r <-> constraint`): `.reify(r)` -- only available
//!   for constraint kinds that implement `NegatableConstraint`, i.e. have
//!   a well-defined negation. `Reification::Reify`.
//!
//! The negatable subset, per `pumpkin_constraints` source, is exactly:
//! `LinearEq`, `LinearNeq`, `LinearLeq`, `LinearLt`, `LinearGeq`,
//! `LinearGt`, `Table`, `Clause`, `Conjunction`. Everything else
//! (`Plus`, `Times`, `Division`, `Absolute`, `Maximum`, `Minimum`,
//! `AllDifferent`, `Cumulative`, `Disjunctive`, `Element`) is
//! post-or-imply-only; requesting `Reify` on one of those is a
//! request-time `ERROR`, not a solver panic, so it should be validated
//! during IR resolution before anything is posted to the solver.
//!
//! # Known gaps vs. the full solver
//!
//! - `Cumulative` / `Disjunctive` durations and demands are plain `i32`
//!   constants in this Pumpkin version -- not variables -- matching
//!   `cumulative`'s and `disjunctive_strict`'s actual signatures. Variable
//!   durations aren't representable here because they aren't representable
//!   in Pumpkin 0.5.0 either.
//! - `satisfy_under_assumptions` (temporarily assuming a set of predicates
//!   true, e.g. for MUS / conflict extraction) isn't modeled by
//!   `SolveMode` yet; it's a different calling convention (assumptions
//!   passed at solve time, not baked into the model) and would be a
//!   separate request shape, not a constraint kind.
//! - Sparse-domain variables (`IntSet`) are included since
//!   `new_sparse_integer` is public API, but no existing tool exercises
//!   them yet -- worth a dedicated test once this is wired up.
//!
//! This module currently defines the IR's *types* only -- the request
//! schema an MCP client would send. Translating it into
//! `pumpkin_solver` calls (name resolution, building `AffineView`s,
//! validating reification legality, dispatching `SolveMode`) is the next
//! step, deliberately left undone until the shape here is agreed on.

use serde::{Deserialize, Serialize};

/// A variable declaration. Every variable has a name, unique within the
/// problem, used to reference it from `constraints` and `solve`.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VarDecl {
    /// An integer variable with a contiguous domain `[min, max]`.
    IntRange { name: String, min: i32, max: i32 },
    /// An integer variable whose domain is an arbitrary finite set of
    /// values (need not be contiguous or sorted).
    IntSet { name: String, values: Vec<i32> },
    /// A boolean decision variable. Usable directly inside `Expr` (as its
    /// 0/1 integer view) as well as `BoolRef` (as a `Literal`, for
    /// clauses/conjunctions/reification).
    Bool { name: String },
}

/// An affine view of a variable: `scale * var + offset`. Free in Pumpkin --
/// no auxiliary variable or propagator is created. A bare reference to
/// `x` is `{"var": "x"}`.
#[derive(Deserialize, schemars::JsonSchema, Clone)]
pub struct Expr {
    /// Name of a declared `IntRange`, `IntSet`, or `Bool` variable.
    pub var: String,
    #[serde(default = "one")]
    pub scale: i32,
    #[serde(default)]
    pub offset: i32,
}

fn one() -> i32 {
    1
}

/// A possibly-negated reference to a `Bool` variable, resolved to a
/// Pumpkin `Literal` (not an integer view). Used in `Clause`,
/// `Conjunction`, and as a reification handle.
#[derive(Deserialize, schemars::JsonSchema, Clone)]
pub struct BoolRef {
    /// Name of a declared `Bool` variable.
    pub var: String,
    /// If true, refers to the negation of the literal.
    #[serde(default)]
    pub negated: bool,
}

/// How a constraint is attached to the model. Omit for an unconditional
/// constraint.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Reification {
    /// Post `literal -> constraint`. Valid for any constraint kind.
    ImpliedBy { literal: BoolRef },
    /// Post `literal <-> constraint`. Only valid for the negatable
    /// constraint kinds -- see the module docs for the exact list;
    /// requesting this on a non-negatable kind is a request-time error.
    Reify { literal: BoolRef },
}

/// One constraint, with an optional reification handle.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct ConstraintEntry {
    #[serde(flatten)]
    pub kind: ConstraintKind,
    #[serde(default)]
    pub reification: Option<Reification>,
}

/// One constraint kind per `pumpkin_constraints` function. See the module
/// docs for which kinds support `Reification::Reify` vs only
/// `Reification::ImpliedBy`.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintKind {
    /// `sum(terms) == rhs`. Negatable.
    LinearEq { terms: Vec<Expr>, rhs: i32 },
    /// `sum(terms) != rhs`. Negatable.
    LinearNeq { terms: Vec<Expr>, rhs: i32 },
    /// `sum(terms) <= rhs`. Negatable.
    LinearLeq { terms: Vec<Expr>, rhs: i32 },
    /// `sum(terms) < rhs`. Negatable.
    LinearLt { terms: Vec<Expr>, rhs: i32 },
    /// `sum(terms) >= rhs`. Negatable.
    LinearGeq { terms: Vec<Expr>, rhs: i32 },
    /// `sum(terms) > rhs`. Negatable.
    LinearGt { terms: Vec<Expr>, rhs: i32 },

    /// `a + b == c`.
    Plus { a: Expr, b: Expr, c: Expr },
    /// `a * b == c`.
    Times { a: Expr, b: Expr, c: Expr },
    /// `numerator / denominator == rhs`, truncating (rounds toward 0).
    /// `denominator`'s domain must not contain 0.
    Division {
        numerator: Expr,
        denominator: Expr,
        rhs: Expr,
    },
    /// `|signed| == absolute`.
    Absolute { signed: Expr, absolute: Expr },
    /// `max(array) == m`.
    Maximum { array: Vec<Expr>, m: Expr },
    /// `min(array) == m`.
    Minimum { array: Vec<Expr>, m: Expr },

    /// All `vars` take pairwise-distinct values.
    AllDifferent { vars: Vec<Expr> },
    /// Cumulative-resource scheduling: at no point in time does total
    /// demand of concurrently-running tasks exceed `capacity`. `starts`,
    /// `durations`, and `demands` must have equal length. `durations` and
    /// `demands` are constants -- Pumpkin 0.5.0 does not support variable
    /// durations/demands.
    Cumulative {
        starts: Vec<Expr>,
        durations: Vec<i32>,
        demands: Vec<i32>,
        capacity: i32,
    },
    /// Unary-resource / no-overlap scheduling: no two tasks' intervals
    /// overlap. `starts` and `durations` must have equal length.
    /// `durations` are constants.
    Disjunctive {
        starts: Vec<Expr>,
        durations: Vec<i32>,
    },
    /// `array[index] == rhs`, 0-based.
    Element {
        array: Vec<Expr>,
        index: Expr,
        rhs: Expr,
    },
    /// If `negated` is false: `vars` must match one row of `tuples`
    /// exactly ("positive table"). If true: `vars` must match none of
    /// `tuples` ("negative table"). Negatable (`negated: false` and
    /// `negated: true` are each other's negation).
    Table {
        vars: Vec<Expr>,
        tuples: Vec<Vec<i32>>,
        #[serde(default)]
        negated: bool,
    },

    /// Disjunction: at least one of `literals` is true. Negatable (its
    /// negation is `Conjunction` over the negated literals).
    Clause { literals: Vec<BoolRef> },
    /// Conjunction: all of `literals` are true. Negatable (its negation
    /// is `Clause` over the negated literals).
    Conjunction { literals: Vec<BoolRef> },
}

/// How to solve the model.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum SolveMode {
    /// Stop at the first feasible solution.
    Satisfy,
    /// Collect up to `max_solutions` distinct feasible solutions.
    Enumerate { max_solutions: usize },
    /// Search for the solution optimising `objective` (must name an
    /// `IntRange` or `IntSet` variable -- Pumpkin's `optimise` takes a
    /// single integer objective variable, not an arbitrary expression;
    /// route a linear combination through an auxiliary variable tied to
    /// it with `LinearEq` if you need to optimise a derived quantity).
    Optimise { objective: String, direction: Direction },
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Minimize,
    Maximize,
}

/// A full problem: variables, constraints tying them together, and how to
/// solve it.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct CspIrProblem {
    pub variables: Vec<VarDecl>,
    #[serde(default)]
    pub constraints: Vec<ConstraintEntry>,
    pub solve: SolveMode,
    /// Wall-clock time limit in seconds before giving up and reporting a
    /// timeout rather than searching indefinitely.
    #[serde(default = "default_time_limit")]
    pub max_time_seconds: u64,
}

fn default_time_limit() -> u64 {
    5
}

#[derive(Serialize)]
pub struct CspIrResponse {
    /// One of "SATISFIABLE", "UNSATISFIABLE", "TIMEOUT", "OPTIMAL", or
    /// "ERROR".
    pub status: String,
    /// Present for "SATISFIABLE" / "OPTIMAL" under `Satisfy` /
    /// `Optimise`: variable name -> assigned value (bools as 0/1).
    pub assignment: Option<std::collections::HashMap<String, i32>>,
    /// Present under `Enumerate`: one assignment map per solution found.
    pub solutions: Option<Vec<std::collections::HashMap<String, i32>>>,
    /// Present for "OPTIMAL": the objective variable's value in the best
    /// solution found.
    pub objective_value: Option<i32>,
    /// Present only when status is "ERROR": what was wrong with the
    /// request (e.g. unknown variable name, duplicate name, `Reify` on a
    /// non-negatable constraint kind, type mismatch between an `Expr`/
    /// `BoolRef` and the referenced variable's declared kind).
    pub error: Option<String>,
}
