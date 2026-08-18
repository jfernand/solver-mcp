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
//! # Interpretation
//!
//! [`solve_csp_ir`] is the interpreter: it resolves variable names to
//! Pumpkin handles (`DomainId` for `IntRange`/`IntSet`, `Literal` for
//! `Bool`), builds every `Expr`/`BoolRef` into the concrete type each
//! constraint constructor expects, validates reification legality, posts
//! everything to a fresh `Solver`, and dispatches `solve.mode`. Every
//! `Expr` is resolved to `AffineView<DomainId>` uniformly -- even a bare
//! `{"var": "x"}` goes through `.scaled(1).offset(0)` -- because Pumpkin's
//! constraint constructors are generic over a single `Var` type and
//! require every term in one call to share the same concrete type; a
//! `Bool` variable's `Expr` form resolves via
//! `Literal::get_integer_variable()` first, landing on the same
//! `AffineView<DomainId>` type as an `IntRange`/`IntSet` reference.
//! Anything wrong with the request (unknown name, duplicate name, a
//! `BoolRef` pointing at a non-`Bool` variable, `scale: 0`, `Reify` on a
//! non-negatable kind, an `Optimise` objective that isn't declared as an
//! int variable) is reported as `CspIrResponse { status: "ERROR", .. }`,
//! never a panic or a mid-build solver mutation the caller can observe.
//! This includes three conditions `pumpkin_constraints` itself documents
//! as panicking rather than erroring -- mismatched `Cumulative`/
//! `Disjunctive` array lengths, and a `Division` `denominator` whose
//! current domain still contains 0 -- which are checked before the
//! corresponding constructor is ever called.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::time::Duration;

use pumpkin_solver::conflict_resolvers::resolvers::ResolutionResolver;
use pumpkin_solver::core::constraints::{Constraint as PkConstraint, NegatableConstraint as PkNegatableConstraint};
use pumpkin_solver::core::optimisation::linear_sat_unsat::LinearSatUnsat;
use pumpkin_solver::core::optimisation::OptimisationDirection;
use pumpkin_solver::core::results::{OptimisationResult, ProblemSolution, SatisfactionResult};
use pumpkin_solver::core::results::solution_iterator::IteratedSolution;
use pumpkin_solver::core::results::SolutionReference;
use pumpkin_solver::core::termination::TimeBudget;
use pumpkin_solver::core::variables::{AffineView, DomainId, Literal, TransformableVariable};
use pumpkin_solver::core::DefaultBrancher;
use pumpkin_solver::propagators::disjunctive::ArgDisjunctiveTask;
use pumpkin_solver::Solver;

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
    /// Present when an objective value is known: always for "OPTIMAL", and
    /// also for "SATISFIABLE" under `Optimise` mode if a feasible (but not
    /// proven-optimal) solution was found before the time limit.
    pub objective_value: Option<i32>,
    /// Present only when status is "ERROR": what was wrong with the
    /// request (e.g. unknown variable name, duplicate name, `Reify` on a
    /// non-negatable constraint kind, type mismatch between an `Expr`/
    /// `BoolRef` and the referenced variable's declared kind).
    pub error: Option<String>,
}

enum ResolvedVar {
    Int(DomainId),
    Bool(Literal),
}

enum ResolvedReification {
    ImpliedBy(Literal),
    Reify(Literal),
}

fn resolve_expr(vars: &HashMap<String, ResolvedVar>, expr: &Expr) -> Result<AffineView<DomainId>, String> {
    if expr.scale == 0 {
        return Err(format!("scale must not be 0 (variable '{}')", expr.var));
    }
    match vars.get(&expr.var) {
        Some(ResolvedVar::Int(id)) => Ok(id.scaled(expr.scale).offset(expr.offset)),
        Some(ResolvedVar::Bool(lit)) => Ok(lit.get_integer_variable().scaled(expr.scale).offset(expr.offset)),
        None => Err(format!("unknown variable '{}'", expr.var)),
    }
}

fn resolve_exprs(vars: &HashMap<String, ResolvedVar>, exprs: &[Expr]) -> Result<Vec<AffineView<DomainId>>, String> {
    exprs.iter().map(|e| resolve_expr(vars, e)).collect()
}

fn resolve_bool(vars: &HashMap<String, ResolvedVar>, r: &BoolRef) -> Result<Literal, String> {
    match vars.get(&r.var) {
        Some(ResolvedVar::Bool(lit)) => Ok(if r.negated { !*lit } else { *lit }),
        Some(ResolvedVar::Int(_)) => Err(format!(
            "'{}' is declared as an int variable, but this position needs a bool",
            r.var
        )),
        None => Err(format!("unknown variable '{}'", r.var)),
    }
}

fn resolve_bools(vars: &HashMap<String, ResolvedVar>, refs: &[BoolRef]) -> Result<Vec<Literal>, String> {
    refs.iter().map(|r| resolve_bool(vars, r)).collect()
}

fn resolve_reification(
    vars: &HashMap<String, ResolvedVar>,
    reification: &Option<Reification>,
) -> Result<Option<ResolvedReification>, String> {
    Ok(match reification {
        None => None,
        Some(Reification::ImpliedBy { literal }) => Some(ResolvedReification::ImpliedBy(resolve_bool(vars, literal)?)),
        Some(Reification::Reify { literal }) => Some(ResolvedReification::Reify(resolve_bool(vars, literal)?)),
    })
}

/// Posts a plain `Constraint` (no defined negation): `None` -> `.post()`,
/// `ImpliedBy` -> `.implied_by()`. `Reify` is a request-time error, since
/// there's nothing for `<->` to negate.
fn post<C: PkConstraint>(solver: &mut Solver, constraint: C, reification: Option<ResolvedReification>) -> Result<(), String> {
    match reification {
        None => {
            solver.add_constraint(constraint).post();
            Ok(())
        }
        Some(ResolvedReification::ImpliedBy(literal)) => {
            solver.add_constraint(constraint).implied_by(literal);
            Ok(())
        }
        Some(ResolvedReification::Reify(_)) => Err(
            "this constraint kind has no defined negation, so it can't be fully reified with \
             `reify`; use `implied_by` instead, or omit `reification` to post it unconditionally"
                .to_string(),
        ),
    }
}

/// Posts a `NegatableConstraint`: `None` -> `.post()`, `ImpliedBy` ->
/// `.implied_by()`, `Reify` -> `.reify()`.
fn post_negatable<C: PkNegatableConstraint>(solver: &mut Solver, constraint: C, reification: Option<ResolvedReification>) {
    match reification {
        None => {
            solver.add_constraint(constraint).post();
        }
        Some(ResolvedReification::ImpliedBy(literal)) => {
            solver.add_constraint(constraint).implied_by(literal);
        }
        Some(ResolvedReification::Reify(literal)) => {
            solver.add_constraint(constraint).reify(literal);
        }
    }
}

fn read_assignment<S: ProblemSolution>(solution: &S, vars: &HashMap<String, ResolvedVar>) -> HashMap<String, i32> {
    vars.iter()
        .map(|(name, resolved)| {
            let value = match resolved {
                ResolvedVar::Int(id) => solution.get_integer_value(*id),
                ResolvedVar::Bool(lit) => i32::from(solution.get_literal_value(*lit)),
            };
            (name.clone(), value)
        })
        .collect()
}

fn post_constraint(
    solver: &mut Solver,
    vars: &HashMap<String, ResolvedVar>,
    kind: ConstraintKind,
    reification: Option<ResolvedReification>,
) -> Result<(), String> {
    match kind {
        ConstraintKind::LinearEq { terms, rhs } => {
            let terms = resolve_exprs(vars, &terms)?;
            let tag = solver.new_constraint_tag();
            post_negatable(solver, pumpkin_constraints::equals(terms, rhs, tag), reification);
        }
        ConstraintKind::LinearNeq { terms, rhs } => {
            let terms = resolve_exprs(vars, &terms)?;
            let tag = solver.new_constraint_tag();
            post_negatable(solver, pumpkin_constraints::not_equals(terms, rhs, tag), reification);
        }
        ConstraintKind::LinearLeq { terms, rhs } => {
            let terms = resolve_exprs(vars, &terms)?;
            let tag = solver.new_constraint_tag();
            post_negatable(solver, pumpkin_constraints::less_than_or_equals(terms, rhs, tag), reification);
        }
        ConstraintKind::LinearLt { terms, rhs } => {
            let terms = resolve_exprs(vars, &terms)?;
            let tag = solver.new_constraint_tag();
            post_negatable(solver, pumpkin_constraints::less_than(terms, rhs, tag), reification);
        }
        ConstraintKind::LinearGeq { terms, rhs } => {
            let terms = resolve_exprs(vars, &terms)?;
            let tag = solver.new_constraint_tag();
            post_negatable(solver, pumpkin_constraints::greater_than_or_equals(terms, rhs, tag), reification);
        }
        ConstraintKind::LinearGt { terms, rhs } => {
            let terms = resolve_exprs(vars, &terms)?;
            let tag = solver.new_constraint_tag();
            post_negatable(solver, pumpkin_constraints::greater_than(terms, rhs, tag), reification);
        }
        ConstraintKind::Plus { a, b, c } => {
            let (a, b, c) = (resolve_expr(vars, &a)?, resolve_expr(vars, &b)?, resolve_expr(vars, &c)?);
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::plus(a, b, c, tag), reification)?;
        }
        ConstraintKind::Times { a, b, c } => {
            let (a, b, c) = (resolve_expr(vars, &a)?, resolve_expr(vars, &b)?, resolve_expr(vars, &c)?);
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::times(a, b, c, tag), reification)?;
        }
        ConstraintKind::Division { numerator, denominator, rhs } => {
            let numerator = resolve_expr(vars, &numerator)?;
            let denominator = resolve_expr(vars, &denominator)?;
            let rhs = resolve_expr(vars, &rhs)?;
            if solver.contains(&denominator, 0) {
                return Err(
                    "division: denominator's domain currently contains 0 -- Pumpkin panics if it \
                     does at solve time, so exclude 0 from its declared domain first"
                        .to_string(),
                );
            }
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::division(numerator, denominator, rhs, tag), reification)?;
        }
        ConstraintKind::Absolute { signed, absolute } => {
            let (signed, absolute) = (resolve_expr(vars, &signed)?, resolve_expr(vars, &absolute)?);
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::absolute(signed, absolute, tag), reification)?;
        }
        ConstraintKind::Maximum { array, m } => {
            let (array, m) = (resolve_exprs(vars, &array)?, resolve_expr(vars, &m)?);
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::maximum(array, m, tag), reification)?;
        }
        ConstraintKind::Minimum { array, m } => {
            let (array, m) = (resolve_exprs(vars, &array)?, resolve_expr(vars, &m)?);
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::minimum(array, m, tag), reification)?;
        }
        ConstraintKind::AllDifferent { vars: names } => {
            let terms = resolve_exprs(vars, &names)?;
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::all_different(terms, tag), reification)?;
        }
        ConstraintKind::Cumulative { starts, durations, demands, capacity } => {
            if starts.len() != durations.len() || starts.len() != demands.len() {
                return Err(format!(
                    "cumulative: starts ({}), durations ({}), and demands ({}) must have equal length",
                    starts.len(),
                    durations.len(),
                    demands.len()
                ));
            }
            let starts = resolve_exprs(vars, &starts)?;
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::cumulative(starts, durations, demands, capacity, tag), reification)?;
        }
        ConstraintKind::Disjunctive { starts, durations } => {
            if starts.len() != durations.len() {
                return Err(format!(
                    "disjunctive: starts ({}) and durations ({}) must have equal length",
                    starts.len(),
                    durations.len()
                ));
            }
            let starts = resolve_exprs(vars, &starts)?;
            let tasks: Vec<_> = starts
                .into_iter()
                .zip(durations)
                .map(|(start_time, processing_time)| ArgDisjunctiveTask { start_time, processing_time })
                .collect();
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::disjunctive_strict(tasks, tag), reification)?;
        }
        ConstraintKind::Element { array, index, rhs } => {
            let array = resolve_exprs(vars, &array)?;
            let index = resolve_expr(vars, &index)?;
            let rhs = resolve_expr(vars, &rhs)?;
            let tag = solver.new_constraint_tag();
            post(solver, pumpkin_constraints::element(index, array, rhs, tag), reification)?;
        }
        ConstraintKind::Table { vars: names, tuples, negated } => {
            let terms = resolve_exprs(vars, &names)?;
            let tag = solver.new_constraint_tag();
            if negated {
                post_negatable(solver, pumpkin_constraints::negative_table(terms, tuples, tag), reification);
            } else {
                post_negatable(solver, pumpkin_constraints::table(terms, tuples, tag), reification);
            }
        }
        ConstraintKind::Clause { literals } => {
            let literals = resolve_bools(vars, &literals)?;
            let tag = solver.new_constraint_tag();
            post_negatable(solver, pumpkin_constraints::clause(literals, tag), reification);
        }
        ConstraintKind::Conjunction { literals } => {
            let literals = resolve_bools(vars, &literals)?;
            let tag = solver.new_constraint_tag();
            post_negatable(solver, pumpkin_constraints::conjunction(literals, tag), reification);
        }
    }
    Ok(())
}

fn build_solver(
    variables: Vec<VarDecl>,
    constraints: Vec<ConstraintEntry>,
) -> Result<(Solver, HashMap<String, ResolvedVar>), String> {
    let mut solver = Solver::default();
    let mut vars: HashMap<String, ResolvedVar> = HashMap::new();

    for decl in variables {
        let (name, resolved) = match decl {
            VarDecl::IntRange { name, min, max } => {
                let id = solver.new_bounded_integer(min, max);
                (name, ResolvedVar::Int(id))
            }
            VarDecl::IntSet { name, values } => {
                let id = solver.new_sparse_integer(values);
                (name, ResolvedVar::Int(id))
            }
            VarDecl::Bool { name } => {
                let lit = solver.new_literal();
                (name, ResolvedVar::Bool(lit))
            }
        };
        if vars.insert(name.clone(), resolved).is_some() {
            return Err(format!("duplicate variable name '{name}'"));
        }
    }

    for entry in constraints {
        let reification = resolve_reification(&vars, &entry.reification)?;
        post_constraint(&mut solver, &vars, entry.kind, reification)?;
    }

    Ok((solver, vars))
}

fn empty_response(status: &str) -> CspIrResponse {
    CspIrResponse {
        status: status.into(),
        assignment: None,
        solutions: None,
        objective_value: None,
        error: None,
    }
}

/// Interprets a [`CspIrProblem`]: resolves names, posts every constraint to
/// a fresh Pumpkin `Solver`, and dispatches `solve.mode`. See the module
/// docs for what "resolves" entails and what gets reported as `ERROR`
/// rather than risking a panic.
pub fn solve_csp_ir(req: CspIrProblem) -> CspIrResponse {
    match solve_csp_ir_inner(req) {
        Ok(response) => response,
        Err(error) => CspIrResponse { error: Some(error), ..empty_response("ERROR") },
    }
}

fn solve_csp_ir_inner(req: CspIrProblem) -> Result<CspIrResponse, String> {
    let CspIrProblem { variables, constraints, solve, max_time_seconds } = req;
    let (mut solver, vars) = build_solver(variables, constraints)?;

    match solve {
        SolveMode::Satisfy => {
            let mut termination = TimeBudget::starting_now(Duration::from_secs(max_time_seconds));
            let mut brancher = solver.default_brancher();
            let mut resolver = ResolutionResolver::default();
            let result = solver.satisfy(&mut brancher, &mut termination, &mut resolver);
            Ok(match result {
                SatisfactionResult::Satisfiable(satisfiable) => {
                    let solution = satisfiable.solution();
                    let assignment = read_assignment(&solution, &vars);
                    CspIrResponse { assignment: Some(assignment), ..empty_response("SATISFIABLE") }
                }
                SatisfactionResult::Unsatisfiable(..) => empty_response("UNSATISFIABLE"),
                SatisfactionResult::Unknown(..) => empty_response("TIMEOUT"),
            })
        }
        SolveMode::Enumerate { max_solutions } => {
            let mut termination = TimeBudget::starting_now(Duration::from_secs(max_time_seconds));
            let mut brancher = solver.default_brancher();
            let mut resolver = ResolutionResolver::default();
            let mut iterator = solver.get_solution_iterator(&mut brancher, &mut termination, &mut resolver);
            let mut solutions = Vec::new();
            let mut timed_out = false;
            while solutions.len() < max_solutions {
                match iterator.next_solution() {
                    IteratedSolution::Solution(solution, _, _, _) => solutions.push(read_assignment(&solution, &vars)),
                    IteratedSolution::Finished | IteratedSolution::Unsatisfiable => break,
                    IteratedSolution::Unknown => {
                        timed_out = true;
                        break;
                    }
                }
            }
            let status = if !solutions.is_empty() {
                "SATISFIABLE"
            } else if timed_out {
                "TIMEOUT"
            } else {
                "UNSATISFIABLE"
            };
            Ok(CspIrResponse { solutions: Some(solutions), ..empty_response(status) })
        }
        SolveMode::Optimise { objective, direction } => {
            let objective_id = match vars.get(&objective) {
                Some(ResolvedVar::Int(id)) => *id,
                Some(ResolvedVar::Bool(_)) => {
                    return Err(format!(
                        "objective '{objective}' is declared as a bool variable; `optimise` needs an int variable"
                    ))
                }
                None => return Err(format!("unknown variable '{objective}'")),
            };
            let direction = match direction {
                Direction::Minimize => OptimisationDirection::Minimise,
                Direction::Maximize => OptimisationDirection::Maximise,
            };
            let mut termination = TimeBudget::starting_now(Duration::from_secs(max_time_seconds));
            let mut brancher = solver.default_brancher();
            let mut resolver = ResolutionResolver::default();
            let callback = |_: &Solver, _: SolutionReference, _: &DefaultBrancher, _: &ResolutionResolver| -> ControlFlow<()> {
                ControlFlow::Continue(())
            };
            let result = solver.optimise(
                &mut brancher,
                &mut termination,
                &mut resolver,
                LinearSatUnsat::new(direction, objective_id, callback),
            );
            Ok(match result {
                OptimisationResult::Optimal(solution) => {
                    let objective_value = Some(solution.get_integer_value(objective_id));
                    let assignment = Some(read_assignment(&solution, &vars));
                    CspIrResponse { assignment, objective_value, ..empty_response("OPTIMAL") }
                }
                OptimisationResult::Satisfiable(solution) => {
                    let objective_value = Some(solution.get_integer_value(objective_id));
                    let assignment = Some(read_assignment(&solution, &vars));
                    CspIrResponse { assignment, objective_value, ..empty_response("SATISFIABLE") }
                }
                OptimisationResult::Unsatisfiable => empty_response("UNSATISFIABLE"),
                OptimisationResult::Unknown => empty_response("TIMEOUT"),
                OptimisationResult::Stopped(..) => unreachable!("the solution callback never requests a stop"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str) -> Expr {
        Expr { var: name.into(), scale: 1, offset: 0 }
    }

    fn scaled(name: &str, scale: i32) -> Expr {
        Expr { var: name.into(), scale, offset: 0 }
    }

    fn lit(name: &str) -> BoolRef {
        BoolRef { var: name.into(), negated: false }
    }

    fn not_lit(name: &str) -> BoolRef {
        BoolRef { var: name.into(), negated: true }
    }

    fn int_range(name: &str, min: i32, max: i32) -> VarDecl {
        VarDecl::IntRange { name: name.into(), min, max }
    }

    fn constraint(kind: ConstraintKind) -> ConstraintEntry {
        ConstraintEntry { kind, reification: None }
    }

    fn problem(variables: Vec<VarDecl>, constraints: Vec<ConstraintEntry>, solve: SolveMode) -> CspIrProblem {
        CspIrProblem { variables, constraints, solve, max_time_seconds: 5 }
    }

    #[test]
    fn satisfy_all_different_feasible() {
        let resp = solve_csp_ir(problem(
            vec![int_range("a", 0, 2), int_range("b", 0, 2), int_range("c", 0, 2)],
            vec![constraint(ConstraintKind::AllDifferent { vars: vec![var("a"), var("b"), var("c")] })],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let assignment = resp.assignment.expect("expected an assignment");
        let mut values: Vec<i32> = assignment.values().copied().collect();
        values.sort();
        assert_eq!(values, vec![0, 1, 2]);
    }

    #[test]
    fn satisfy_all_different_infeasible() {
        let resp = solve_csp_ir(problem(
            vec![int_range("a", 0, 1), int_range("b", 0, 1), int_range("c", 0, 1)],
            vec![constraint(ConstraintKind::AllDifferent { vars: vec![var("a"), var("b"), var("c")] })],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "UNSATISFIABLE");
        assert!(resp.assignment.is_none());
    }

    #[test]
    fn expr_offset_encodes_n_queens_diagonals() {
        // 4-queens: solvable (e.g. columns [1, 3, 0, 2]), and every solution
        // must respect both diagonal all-different constraints built purely
        // from Expr's offset -- no auxiliary variables.
        let queens: Vec<String> = (0..4).map(|i| format!("q{i}")).collect();
        let variables = queens.iter().map(|n| int_range(n, 0, 3)).collect();
        let plain: Vec<Expr> = queens.iter().map(|n| var(n)).collect();
        let diag_up: Vec<Expr> = queens
            .iter()
            .enumerate()
            .map(|(i, n)| Expr { var: n.clone(), scale: 1, offset: i as i32 })
            .collect();
        let diag_down: Vec<Expr> = queens
            .iter()
            .enumerate()
            .map(|(i, n)| Expr { var: n.clone(), scale: 1, offset: -(i as i32) })
            .collect();

        let resp = solve_csp_ir(problem(
            variables,
            vec![
                constraint(ConstraintKind::AllDifferent { vars: plain }),
                constraint(ConstraintKind::AllDifferent { vars: diag_up }),
                constraint(ConstraintKind::AllDifferent { vars: diag_down }),
            ],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let assignment = resp.assignment.expect("expected an assignment");
        let cols: Vec<i32> = (0..4).map(|i| assignment[&format!("q{i}")]).collect();
        let mut sorted = cols.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3], "columns must be pairwise distinct");
        for i in 0..4 {
            for j in (i + 1)..4 {
                assert_ne!(cols[i] - cols[j], (i as i32) - (j as i32), "diagonal conflict");
                assert_ne!(cols[i] - cols[j], -((i as i32) - (j as i32)), "diagonal conflict");
            }
        }
    }

    #[test]
    fn send_more_money_solves_to_the_known_answer() {
        let letters = ["S", "E", "N", "D", "M", "O", "R", "Y"];
        let variables: Vec<VarDecl> = letters
            .iter()
            .map(|l| {
                let min = if *l == "S" || *l == "M" { 1 } else { 0 };
                int_range(l, min, 9)
            })
            .collect();

        let terms = vec![
            scaled("S", 1000),
            scaled("E", 91),
            scaled("N", -90),
            scaled("D", 1),
            scaled("M", -9000),
            scaled("O", -900),
            scaled("R", 10),
            scaled("Y", -1),
        ];

        let resp = solve_csp_ir(problem(
            variables,
            vec![
                constraint(ConstraintKind::AllDifferent { vars: letters.iter().map(|l| var(l)).collect() }),
                constraint(ConstraintKind::LinearEq { terms, rhs: 0 }),
            ],
            SolveMode::Satisfy,
        ));

        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        let send = 1000 * a["S"] + 100 * a["E"] + 10 * a["N"] + a["D"];
        let more = 1000 * a["M"] + 100 * a["O"] + 10 * a["R"] + a["E"];
        let money = 10000 * a["M"] + 1000 * a["O"] + 100 * a["N"] + 10 * a["E"] + a["Y"];
        assert_eq!(send + more, money);
        assert_ne!(a["S"], 0);
        assert_ne!(a["M"], 0);
        let mut digits: Vec<i32> = letters.iter().map(|l| a[*l]).collect();
        digits.sort();
        digits.dedup();
        assert_eq!(digits.len(), 8, "letters must map to distinct digits");
    }

    #[test]
    fn table_constraint_restricts_to_listed_tuples() {
        let resp = solve_csp_ir(problem(
            vec![int_range("meal", 0, 2), int_range("drink", 0, 2)],
            vec![
                constraint(ConstraintKind::Table {
                    vars: vec![var("meal"), var("drink")],
                    tuples: vec![vec![0, 0], vec![0, 2], vec![1, 1], vec![2, 2], vec![2, 0]],
                    negated: false,
                }),
                constraint(ConstraintKind::LinearNeq { terms: vec![var("drink")], rhs: 0 }),
            ],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        // The only tuple with drink != White(0) is (Steak=1, Red=1).
        assert_eq!((a["meal"], a["drink"]), (1, 1));
    }

    #[test]
    fn implied_by_only_constrains_when_literal_is_true() {
        // trigger -> (x == 1). With trigger forced false, x is free.
        let resp = solve_csp_ir(problem(
            vec![VarDecl::Bool { name: "trigger".into() }, int_range("x", 0, 5)],
            vec![
                ConstraintEntry {
                    kind: ConstraintKind::LinearEq { terms: vec![var("x")], rhs: 1 },
                    reification: Some(Reification::ImpliedBy { literal: lit("trigger") }),
                },
                constraint(ConstraintKind::LinearEq { terms: vec![var("trigger")], rhs: 0 }),
                constraint(ConstraintKind::LinearNeq { terms: vec![var("x")], rhs: 1 }),
            ],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!(a["trigger"], 0);
        assert_ne!(a["x"], 1);
    }

    #[test]
    fn reify_ties_literal_to_constraint_in_both_directions() {
        // r <-> (x == 3). Forcing r true must force x == 3.
        let resp = solve_csp_ir(problem(
            vec![VarDecl::Bool { name: "r".into() }, int_range("x", 0, 5)],
            vec![
                ConstraintEntry {
                    kind: ConstraintKind::LinearEq { terms: vec![var("x")], rhs: 3 },
                    reification: Some(Reification::Reify { literal: lit("r") }),
                },
                constraint(ConstraintKind::LinearEq { terms: vec![var("r")], rhs: 1 }),
            ],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!(a["x"], 3);
    }

    #[test]
    fn clause_and_conjunction_drive_enumeration_count() {
        // (a \/ b) and NOT(c AND d) -- i.e. clause(a, b) and clause(!c, !d).
        // Of 16 assignments, a=b=0 fails (4), and among the rest c=d=1 fails
        // (3 more), leaving 9.
        let resp = solve_csp_ir(problem(
            vec![
                VarDecl::Bool { name: "a".into() },
                VarDecl::Bool { name: "b".into() },
                VarDecl::Bool { name: "c".into() },
                VarDecl::Bool { name: "d".into() },
            ],
            vec![
                constraint(ConstraintKind::Clause { literals: vec![lit("a"), lit("b")] }),
                constraint(ConstraintKind::Clause { literals: vec![not_lit("c"), not_lit("d")] }),
            ],
            SolveMode::Enumerate { max_solutions: 100 },
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let solutions = resp.solutions.expect("expected solutions");
        assert_eq!(solutions.len(), 9);
    }

    #[test]
    fn optimise_maximizes_knapsack_value() {
        let resp = solve_csp_ir(problem(
            vec![
                VarDecl::Bool { name: "take_a".into() },
                VarDecl::Bool { name: "take_b".into() },
                VarDecl::Bool { name: "take_c".into() },
                VarDecl::Bool { name: "take_d".into() },
                int_range("total_value", 0, 20),
            ],
            vec![
                constraint(ConstraintKind::LinearLeq {
                    terms: vec![scaled("take_a", 2), scaled("take_b", 3), scaled("take_c", 4), scaled("take_d", 5)],
                    rhs: 8,
                }),
                constraint(ConstraintKind::LinearEq {
                    terms: vec![
                        scaled("take_a", 3),
                        scaled("take_b", 4),
                        scaled("take_c", 5),
                        scaled("take_d", 8),
                        scaled("total_value", -1),
                    ],
                    rhs: 0,
                }),
            ],
            SolveMode::Optimise { objective: "total_value".into(), direction: Direction::Maximize },
        ));
        assert_eq!(resp.status, "OPTIMAL");
        assert_eq!(resp.objective_value, Some(12));
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!(a["take_b"], 1);
        assert_eq!(a["take_d"], 1);
        assert_eq!(a["take_a"], 0);
        assert_eq!(a["take_c"], 0);
    }

    #[test]
    fn arithmetic_chain_times_division_absolute_min_max() {
        let resp = solve_csp_ir(problem(
            vec![
                int_range("a", 1, 9),
                int_range("b", 1, 9),
                int_range("c", 1, 20),
                int_range("diff", -8, 8),
                int_range("three", 3, 3),
                int_range("q", 2, 2),
            ],
            vec![
                constraint(ConstraintKind::Times { a: var("a"), b: var("b"), c: var("c") }),
                constraint(ConstraintKind::Division { numerator: var("b"), denominator: var("a"), rhs: var("q") }),
                constraint(ConstraintKind::LinearEq {
                    terms: vec![var("a"), scaled("b", -1), scaled("diff", -1)],
                    rhs: 0,
                }),
                constraint(ConstraintKind::Absolute { signed: var("diff"), absolute: var("three") }),
                constraint(ConstraintKind::Maximum { array: vec![var("a"), var("b"), var("c")], m: var("c") }),
                constraint(ConstraintKind::Minimum { array: vec![var("a"), var("b"), var("c")], m: var("a") }),
            ],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!((a["a"], a["b"], a["c"]), (3, 6, 18));
    }

    #[test]
    fn unknown_variable_is_a_clean_error_not_a_panic() {
        let resp = solve_csp_ir(problem(
            vec![int_range("a", 0, 5)],
            vec![constraint(ConstraintKind::LinearEq { terms: vec![var("nope")], rhs: 0 })],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "ERROR");
        assert!(resp.error.unwrap().contains("nope"));
    }

    #[test]
    fn duplicate_variable_name_is_an_error() {
        let resp = solve_csp_ir(problem(
            vec![int_range("a", 0, 5), int_range("a", 0, 5)],
            vec![],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "ERROR");
        assert!(resp.error.unwrap().contains("duplicate"));
    }

    #[test]
    fn reify_on_non_negatable_kind_is_an_error() {
        let resp = solve_csp_ir(problem(
            vec![VarDecl::Bool { name: "r".into() }, int_range("a", 0, 5), int_range("b", 0, 5)],
            vec![ConstraintEntry {
                kind: ConstraintKind::Plus { a: var("a"), b: var("b"), c: var("a") },
                reification: Some(Reification::Reify { literal: lit("r") }),
            }],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "ERROR");
        assert!(resp.error.unwrap().contains("negation"));
    }

    #[test]
    fn bool_ref_on_int_variable_is_a_type_error() {
        let resp = solve_csp_ir(problem(
            vec![int_range("a", 0, 5)],
            vec![constraint(ConstraintKind::Clause { literals: vec![lit("a")] })],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "ERROR");
        assert!(resp.error.unwrap().contains("bool"));
    }

    #[test]
    fn division_by_a_domain_that_can_be_zero_is_a_clean_error() {
        let resp = solve_csp_ir(problem(
            vec![int_range("num", 0, 5), int_range("den", 0, 5), int_range("q", 0, 5)],
            vec![constraint(ConstraintKind::Division { numerator: var("num"), denominator: var("den"), rhs: var("q") })],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "ERROR");
        assert!(resp.error.unwrap().contains("denominator"));
    }

    #[test]
    fn mismatched_cumulative_lengths_are_a_clean_error() {
        let resp = solve_csp_ir(problem(
            vec![int_range("s0", 0, 10), int_range("s1", 0, 10)],
            vec![constraint(ConstraintKind::Cumulative {
                starts: vec![var("s0"), var("s1")],
                durations: vec![2],
                demands: vec![1, 1],
                capacity: 1,
            })],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "ERROR");
        assert!(resp.error.unwrap().contains("equal length"));
    }

    #[test]
    fn disjunctive_prevents_overlap() {
        let resp = solve_csp_ir(problem(
            vec![int_range("s0", 0, 20), int_range("s1", 0, 20)],
            vec![constraint(ConstraintKind::Disjunctive { starts: vec![var("s0"), var("s1")], durations: vec![5, 5] })],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        assert!(a["s0"] + 5 <= a["s1"] || a["s1"] + 5 <= a["s0"], "tasks must not overlap");
    }

    #[test]
    fn element_looks_up_array_by_index() {
        let resp = solve_csp_ir(problem(
            vec![
                int_range("row0", 10, 10),
                int_range("row1", 20, 20),
                int_range("row2", 30, 30),
                int_range("index", 0, 2),
                int_range("looked_up", 0, 30),
            ],
            vec![
                constraint(ConstraintKind::Element {
                    array: vec![var("row0"), var("row1"), var("row2")],
                    index: var("index"),
                    rhs: var("looked_up"),
                }),
                constraint(ConstraintKind::LinearEq { terms: vec![var("index")], rhs: 1 }),
            ],
            SolveMode::Satisfy,
        ));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!(a["looked_up"], 20);
    }

    /// Loads the fenced ```json block out of `examples/<filename>` and
    /// parses it as a `CspIrProblem`, so the worked examples in the repo's
    /// `examples/` folder are checked against the real interpreter, not
    /// just hand-derived. Used by the `example_*` tests below.
    fn load_example(filename: &str) -> CspIrProblem {
        let path = format!("{}/examples/{}", env!("CARGO_MANIFEST_DIR"), filename);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
        let start = text.find("```json\n").unwrap_or_else(|| panic!("{filename}: no json block found")) + "```json\n".len();
        let end = start + text[start..].find("\n```").unwrap_or_else(|| panic!("{filename}: unterminated json block"));
        serde_json::from_str(&text[start..end]).unwrap_or_else(|e| panic!("{filename}: invalid IR JSON: {e}"))
    }

    #[test]
    fn example_01_send_more_money() {
        let resp = solve_csp_ir(load_example("01_send_more_money.md"));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        let send = 1000 * a["S"] + 100 * a["E"] + 10 * a["N"] + a["D"];
        let more = 1000 * a["M"] + 100 * a["O"] + 10 * a["R"] + a["E"];
        let money = 10000 * a["M"] + 1000 * a["O"] + 100 * a["N"] + 10 * a["E"] + a["Y"];
        assert_eq!(send + more, money);
        assert_ne!(a["S"], 0);
        assert_ne!(a["M"], 0);
    }

    #[test]
    fn example_02_n_queens() {
        let resp = solve_csp_ir(load_example("02_n_queens.md"));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        let cols: Vec<i32> = (0..8).map(|i| a[&format!("queens_{i}")]).collect();
        let mut sorted = cols.clone();
        sorted.sort();
        assert_eq!(sorted, (0..8).collect::<Vec<_>>(), "columns must be pairwise distinct");
        for i in 0..8 {
            for j in (i + 1)..8 {
                let row_gap = (i as i32) - (j as i32);
                assert_ne!(cols[i] - cols[j], row_gap, "diagonal conflict between rows {i} and {j}");
                assert_ne!(cols[i] - cols[j], -row_gap, "diagonal conflict between rows {i} and {j}");
            }
        }
    }

    #[test]
    fn example_03_traveling_salesman_finds_the_optimal_80_tour() {
        let resp = solve_csp_ir(load_example("03_traveling_salesman.md"));
        assert_eq!(resp.status, "OPTIMAL");
        assert_eq!(resp.objective_value, Some(80));
        let a = resp.assignment.expect("expected an assignment");
        // The successor relation must be a single 4-cycle through all
        // cities, not two disjoint 2-cycles -- exactly what MTZ rules out.
        let mut visited = [false; 4];
        let mut city = 0;
        for _ in 0..4 {
            assert!(!visited[city], "revisited city {city} before completing the tour");
            visited[city] = true;
            city = a[&format!("next_{city}")] as usize;
        }
        assert_eq!(city, 0, "tour must return to the start after visiting all 4 cities");
    }

    #[test]
    fn example_04_house_construction_schedule() {
        let resp = solve_csp_ir(load_example("04_house_construction_schedule.md"));
        assert_eq!(resp.status, "OPTIMAL");
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!(a["painting_start"] + 2, a["makespan"]);
        assert!(a["framing_start"] >= a["foundation_start"] + 4);
        assert!(a["plumbing_start"] >= a["framing_start"] + 6);
        assert!(a["electrical_start"] >= a["framing_start"] + 6);
        assert!(a["drywall_start"] >= a["plumbing_start"] + 3);
        assert!(a["drywall_start"] >= a["electrical_start"] + 3);
        assert!(a["painting_start"] >= a["drywall_start"] + 4);
    }

    #[test]
    fn example_05_mini_sudoku() {
        let resp = solve_csp_ir(load_example("05_mini_sudoku.md"));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!(a["cell_0_0"], 1);
        assert_eq!(a["cell_0_3"], 4);
        assert_eq!(a["cell_1_2"], 1);
        assert_eq!(a["cell_2_1"], 1);
        assert_eq!(a["cell_3_0"], 4);
        assert_eq!(a["cell_3_3"], 1);
        for r in 0..4 {
            let mut row: Vec<i32> = (0..4).map(|c| a[&format!("cell_{r}_{c}")]).collect();
            row.sort();
            assert_eq!(row, vec![1, 2, 3, 4], "row {r} must contain each digit once");
        }
        for c in 0..4 {
            let mut col: Vec<i32> = (0..4).map(|r| a[&format!("cell_{r}_{c}")]).collect();
            col.sort();
            assert_eq!(col, vec![1, 2, 3, 4], "column {c} must contain each digit once");
        }
    }

    #[test]
    fn example_06_dinner_pairing_table() {
        let resp = solve_csp_ir(load_example("06_dinner_pairing_table.md"));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!((a["meal"], a["drink"]), (1, 1), "expected Steak(1) + Red(1)");
    }

    #[test]
    fn example_07_knapsack_optimization() {
        let resp = solve_csp_ir(load_example("07_knapsack_optimization.md"));
        assert_eq!(resp.status, "OPTIMAL");
        assert_eq!(resp.objective_value, Some(12));
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!((a["take_A"], a["take_B"], a["take_C"], a["take_D"]), (0, 1, 0, 1));
    }

    #[test]
    fn example_08_arithmetic_puzzle() {
        let resp = solve_csp_ir(load_example("08_arithmetic_puzzle.md"));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        assert_eq!((a["a"], a["b"], a["c"]), (3, 6, 18));
    }

    #[test]
    fn example_09_job_sequencing() {
        let resp = solve_csp_ir(load_example("09_job_sequencing.md"));
        assert_eq!(resp.status, "SATISFIABLE");
        let a = resp.assignment.expect("expected an assignment");
        let durations = [3, 5, 2, 4];
        let starts: Vec<i32> = (1..=4).map(|i| a[&format!("start_{i}")]).collect();
        for i in 0..4 {
            for j in (i + 1)..4 {
                assert!(
                    starts[i] + durations[i] <= starts[j] || starts[j] + durations[j] <= starts[i],
                    "jobs {} and {} overlap",
                    i + 1,
                    j + 1
                );
            }
        }
        assert!(starts[1] > 1, "job 2 must start after time 1");
        assert!(starts[3] <= 6, "job 4 must finish by time 10");
    }

    #[test]
    fn example_10_dinner_party_logic() {
        let resp = solve_csp_ir(load_example("10_dinner_party_logic.md"));
        assert_eq!(resp.status, "SATISFIABLE");
        let solutions = resp.solutions.expect("expected solutions");
        assert_eq!(solutions.len(), 15);
        for s in &solutions {
            assert!(s["attends_alice"] == 1 || s["attends_bob"] == 1);
            assert!(s["attends_carol"] == 0 || s["attends_dana"] == 1);
            if s["dana_after_party"] == 1 {
                assert_eq!(s["attends_dana"], 1);
            }
            let all_attend = s["attends_alice"] == 1 && s["attends_bob"] == 1 && s["attends_carol"] == 1 && s["attends_dana"] == 1;
            assert_eq!(s["full_house"] == 1, all_attend);
        }
    }
}
