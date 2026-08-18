mod csp_ir;
mod csp_tools;
mod lp_tools;

use csp_ir::{CspIrProblem, solve_csp_ir};
use csp_tools::{
    CspRequest, GroupedCspRequest, ScheduleRequest, solve_csp, solve_grouped_csp, solve_scheduling,
};
use lp_tools::{AssignmentRequest, LpRequest, solve_assignment, solve_lp};
use rmcp::ErrorData as McpError;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};

/// The optimization brain: a self-contained MCP server exposing constraint
/// satisfaction (Pumpkin) and linear programming (microlp) as tools. Both
/// solvers are pure Rust -- no FFI, no system libraries, single static
/// binary. Callers supply structured problem data (matrices, bounds,
/// constraints); this server never fetches its own data (addresses,
/// calendars, etc.) -- that's left to whatever other MCP servers an agent
/// composes this with.
#[derive(Clone)]
struct SolverServer {
    tool_router: ToolRouter<SolverServer>,
}

impl SolverServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl SolverServer {
    /// Solves a general constraint-satisfaction problem over bounded integer
    /// variables, optionally with an all-different constraint. Returns any
    /// valid assignment, not an optimized one. Use for combinatorial
    /// puzzles, exclusion-based assignment, and feasibility checks --
    /// "does a valid arrangement exist at all", not "what's the best one".
    #[tool]
    async fn solve_csp(
        &self,
        Parameters(req): Parameters<CspRequest>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&solve_csp(req)).unwrap(),
        )]))
    }

    /// Solves a constraint-satisfaction problem expressed as named, grouped
    /// variables -- e.g. Color/Nationality/Drink/Smoke/Pet with five values
    /// each -- tied together by relations: equality (a == b), directional
    /// offset (a == b + k), undirected distance (|a - b| == k), and
    /// constants (a == k). Each group gets its own all-different constraint,
    /// so values from different groups are never incorrectly forced apart
    /// the way a single global all-different would. Use this for logic-grid
    /// puzzles like the classic Zebra Puzzle, where clues relate named
    /// attributes rather than an undifferentiated pool of integers.
    #[tool]
    async fn solve_grouped_csp(
        &self,
        Parameters(req): Parameters<GroupedCspRequest>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&solve_grouped_csp(req)).unwrap(),
        )]))
    }

    /// Solves resource-constrained scheduling: given tasks with fixed
    /// durations and resource demands sharing one finite-capacity resource,
    /// finds start times so usage never exceeds capacity at any instant.
    /// Use for machine scheduling, job sequencing on shared equipment, or
    /// any single-resource rostering problem.
    #[tool]
    async fn solve_scheduling(
        &self,
        Parameters(req): Parameters<ScheduleRequest>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&solve_scheduling(req)).unwrap(),
        )]))
    }

    /// Solves a constraint-satisfaction or optimisation problem expressed
    /// in the general CSP intermediate representation: named int/bool
    /// variables, a flat list of constraints (one kind per Pumpkin
    /// constraint primitive -- linear (in)equalities, arithmetic
    /// (plus/times/division/absolute/min/max), all-different, cumulative
    /// and disjunctive scheduling, element, table, and boolean
    /// clause/conjunction, each optionally half- or fully-reified against
    /// a boolean literal), and a solve directive (first solution, several
    /// distinct solutions, or optimise an objective variable). Use this
    /// instead of `solve_csp`/`solve_grouped_csp`/`solve_scheduling` for
    /// anything those narrower tools can't express -- reified/conditional
    /// constraints, table constraints, disjunctive (no-overlap)
    /// scheduling, arithmetic decompositions, or optimisation over a CP
    /// (not LP) model. See `examples/` in the repo for worked problems
    /// (SEND+MORE=MONEY, N-Queens, TSP, job-shop scheduling, knapsack,
    /// Sudoku, and more) paired with their IR formulation.
    #[tool]
    async fn solve_csp_ir(
        &self,
        Parameters(req): Parameters<CspIrProblem>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&solve_csp_ir(req)).unwrap(),
        )]))
    }

    /// Solves a linear program: minimizes a linear objective subject to
    /// linear (in)equality constraints over bounded continuous variables.
    /// Use for resource allocation, blending, and cost-minimization
    /// problems -- anything where fractional variable values are
    /// meaningful and all relationships are linear.
    #[tool]
    async fn solve_lp(
        &self,
        Parameters(req): Parameters<LpRequest>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&solve_lp(req)).unwrap(),
        )]))
    }

    /// Solves a one-to-one assignment problem: given a square cost matrix
    /// between N agents and N tasks, finds the assignment minimizing (or
    /// maximizing) total cost, with each agent getting exactly one task.
    /// Use for worker-to-task, order-to-warehouse, or ad-to-slot matching.
    #[tool]
    async fn solve_assignment(
        &self,
        Parameters(req): Parameters<AssignmentRequest>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&solve_assignment(req)).unwrap(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for SolverServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::new("solver-mcp", env!("CARGO_PKG_VERSION")))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = SolverServer::new();
    let service = server
        .serve(stdio())
        .await?;
    service
        .waiting()
        .await?;
    Ok(())
}
