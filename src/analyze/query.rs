//! Runtime Declarative Query Layer (Datalog)
//!
//! A lightweight, budget-bounded Datalog evaluator for runtime graph queries.
//! AI agents can write precise structural queries (e.g. "Find all methods that
//! call a specific function but are not registered in the DI container") in a
//! single, token-efficient call.
//!
//! Phases 1-4: EDB Loader, Parser, Semi-Naive Evaluator, Safety Guardrails
//! Phase 6: Tests. Phase 5 (MCP/CLI) lives in handlers.rs and main.rs.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// Embedded Datalog standard library — compiled into the binary.
const STDLIB_SOURCE: &str = include_str!("query/stdlib.dl");

/// Empty relation singleton for unknown relation names.

// ═══════════════════════════════════════════════════════════════════════════════
// Phase 1: EDB Relational Loader — data types and extensional database
// ═══════════════════════════════════════════════════════════════════════════════

/// A single value in a relation tuple.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub enum Val {
    Str(String),
    Int(i64),
}

impl std::fmt::Display for Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Val::Str(s) => write!(f, "\"{}\"", s),
            Val::Int(n) => write!(f, "{}", n),
        }
    }
}

/// A Datalog term: either a variable or a constant value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Term {
    Var(String),
    Const(Val),
}

/// A relational atom, e.g., `edge(X, Y, "calls", _)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Atom {
    pub name: String,
    pub terms: Vec<Term>,
    pub negated: bool,
}

/// A Datalog rule: `Head :- Body1, Body2, !Body3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rule {
    pub head: Atom,
    pub body: Vec<Atom>,
}

/// Extensional Database: flat facts populated from the GrapheniumGraph.
#[derive(Default, Debug, Clone)]
pub struct Edb {
    pub relations: HashMap<String, HashSet<Vec<Val>>>,
}

impl Edb {
    /// Load facts directly from a GrapheniumGraph.
    pub fn load_from_graph(&mut self, graph: &crate::model::GrapheniumGraph) {
        if cfg!(test) {
            eprintln!(
                "DEBUG load_from_graph: {} nodes, {} edges",
                graph.node_count(),
                graph.edges_iter().count()
            );
        }
        // 1. node(Id, Label, Type, File, Community)
        let mut nodes = HashSet::new();
        for n in graph.nodes() {
            nodes.insert(vec![
                Val::Str(n.id.clone()),
                Val::Str(n.label.clone()),
                Val::Str(format!("{:?}", n.file_type)),
                Val::Str(n.source_file.clone()),
                Val::Int(n.community.unwrap_or(999) as i64),
            ]);
        }
        self.relations.insert("node".to_string(), nodes);

        // 2. Typed relations: calls, imports, contains, inherits, implements
        let mut calls = HashSet::new();
        let mut imports = HashSet::new();
        let mut contains = HashSet::new();
        let mut inherits = HashSet::new();
        let mut implements = HashSet::new();
        let mut edges = HashSet::new();
        for e in graph.edges_iter() {
            let conf = Val::Str(format!("{:?}", e.confidence));
            let tuple = vec![
                Val::Str(e.source.clone()),
                Val::Str(e.target.clone()),
                conf.clone(),
            ];
            match e.relation.as_str() {
                "calls" => {
                    calls.insert(tuple);
                }
                "imports" => {
                    imports.insert(tuple);
                }
                "contains" => {
                    contains.insert(tuple);
                }
                "inherits" => {
                    inherits.insert(tuple);
                }
                "implements" => {
                    implements.insert(tuple);
                }
                _ => {}
            }
            edges.insert(vec![
                Val::Str(e.source.clone()),
                Val::Str(e.target.clone()),
                Val::Str(e.relation.clone()),
                conf,
            ]);
        }
        self.relations.insert("calls".to_string(), calls);
        self.relations.insert("imports".to_string(), imports);
        self.relations.insert("contains".to_string(), contains);
        self.relations.insert("inherits".to_string(), inherits);
        self.relations.insert("implements".to_string(), implements);
        self.relations.insert("edge".to_string(), edges);

        // 3. degree(NodeId, Count) and hub(NodeId) for is_hub/1
        let mut degrees = HashSet::new();
        let mut hubs = HashSet::new();
        for n in graph.nodes() {
            let count = graph.degree(&n.id) as i64;
            degrees.insert(vec![Val::Str(n.id.clone()), Val::Int(count)]);
            if count > 15 {
                hubs.insert(vec![Val::Str(n.id.clone())]);
            }
        }
        self.relations.insert("degree".to_string(), degrees);
        self.relations.insert("hub".to_string(), hubs);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Phase 2: Datalog Lexer & Parser
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Ident(String),
    StringLiteral(String),
    IntLiteral(i64),
    Arrow,      // :-
    Query,      // ?-
    Comma,      // ,
    LParen,     // (
    RParen,     // )
    Dot,        // .
    Not,        // !
    Underscore, // _
}

/// A parsed Datalog program.
#[derive(Default, Debug)]
pub struct DatalogProgram {
    pub facts: Vec<Atom>,
    pub rules: Vec<Rule>,
    pub goal: Vec<Atom>,
}

impl DatalogProgram {
    /// Merge standard library rules into the parsed program.
    /// Stdlib rules are prepended so they are evaluated before user rules.
    pub fn merge_stdlib(&mut self, stdlib_rules: Vec<Rule>) {
        let mut combined = stdlib_rules;
        combined.append(&mut self.rules);
        self.rules = combined;
    }
}

/// Parse a complete Datalog program from source text.
pub fn parse_datalog_program(source: &str) -> Result<DatalogProgram, String> {
    let tokens = tokenize(source)?;
    parse(&tokens)
}

static STDLIB_RULES: OnceLock<Arc<Vec<Rule>>> = OnceLock::new();

/// Return cached standard-library rules (parsed once at first use).
pub fn stdlib_rules() -> Arc<Vec<Rule>> {
    STDLIB_RULES
        .get_or_init(|| {
            let program = parse_datalog_program(STDLIB_SOURCE)
                .expect("embedded Datalog standard library must parse successfully");
            Arc::new(program.rules)
        })
        .clone()
}

/// Tokenize a Datalog query string.
pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip whitespace and comments
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        if chars[i] == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        match chars[i] {
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            '.' => {
                tokens.push(Token::Dot);
                i += 1;
            }
            '!' => {
                tokens.push(Token::Not);
                i += 1;
            }
            '_' => {
                tokens.push(Token::Underscore);
                i += 1;
            }
            '?' => {
                if i + 1 < chars.len() && chars[i + 1] == '-' {
                    tokens.push(Token::Query);
                    i += 2;
                } else {
                    return Err(format!("Unexpected '?' at position {}", i));
                }
            }
            ':' => {
                if i + 1 < chars.len() && chars[i + 1] == '-' {
                    tokens.push(Token::Arrow);
                    i += 2;
                } else {
                    return Err(format!("Unexpected ':' at position {}", i));
                }
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    s.push(chars[i]);
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("Unterminated string literal".to_string());
                }
                i += 1; // closing "
                tokens.push(Token::StringLiteral(s));
            }
            c if c.is_ascii_digit()
                || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) =>
            {
                let mut s = String::new();
                s.push(chars[i]);
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    s.push(chars[i]);
                    i += 1;
                }
                let n: i64 = s.parse().map_err(|_| format!("Invalid integer: {}", s))?;
                tokens.push(Token::IntLiteral(n));
            }
            c if c.is_ascii_alphabetic() => {
                let mut s = String::new();
                s.push(chars[i]);
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    s.push(chars[i]);
                    i += 1;
                }
                tokens.push(Token::Ident(s));
            }
            c => return Err(format!("Unexpected character '{}' at position {}", c, i)),
        }
    }

    Ok(tokens)
}

/// Parse a Datalog program from tokens.
pub fn parse(tokens: &[Token]) -> Result<DatalogProgram, String> {
    let mut p = Parser {
        tokens,
        pos: 0,
        anon_counter: 0,
    };
    p.parse_program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    anon_counter: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a Token> {
        let tok = self.tokens.get(self.pos);
        self.pos += 1;
        tok
    }

    fn consume(&mut self, expected: &Token) -> Result<(), String> {
        match self.tokens.get(self.pos) {
            Some(t) if t == expected => {
                self.pos += 1;
                Ok(())
            }
            Some(t) => Err(format!("Expected {:?}, got {:?}", expected, t)),
            None => Err(format!("Expected {:?}, got end of input", expected)),
        }
    }

    fn parse_program(&mut self) -> Result<DatalogProgram, String> {
        let mut program = DatalogProgram::default();
        while self.pos < self.tokens.len() {
            if self.peek() == Some(&Token::Query) {
                self.consume(&Token::Query)?;
                let mut goals = Vec::new();
                loop {
                    goals.push(self.parse_atom()?);
                    if self.peek() == Some(&Token::Comma) {
                        self.consume(&Token::Comma)?;
                    } else {
                        break;
                    }
                }
                self.consume(&Token::Dot)?;
                program.goal = goals;
            } else {
                let atom = self.parse_atom()?;
                if self.peek() == Some(&Token::Arrow) {
                    self.consume(&Token::Arrow)?;
                    let mut body = Vec::new();
                    loop {
                        body.push(self.parse_atom()?);
                        if self.peek() == Some(&Token::Comma) {
                            self.consume(&Token::Comma)?;
                        } else {
                            break;
                        }
                    }
                    self.consume(&Token::Dot)?;
                    program.rules.push(Rule { head: atom, body });
                } else {
                    self.consume(&Token::Dot)?;
                    program.facts.push(atom);
                }
            }
        }
        Ok(program)
    }

    fn parse_atom(&mut self) -> Result<Atom, String> {
        let negated = match self.peek() {
            Some(&Token::Not) => {
                self.consume(&Token::Not)?;
                true
            }
            Some(Token::Ident(ref s)) if s == "not" => {
                self.consume(&Token::Ident(s.clone()))?;
                true
            }
            _ => false,
        };

        let name = match self.next() {
            Some(Token::Ident(s)) => s.clone(),
            Some(t) => return Err(format!("Expected relation identifier, got {:?}", t)),
            None => return Err("Expected relation identifier, got end of input".to_string()),
        };

        self.consume(&Token::LParen)?;
        let mut terms = Vec::new();
        loop {
            match self.next() {
                Some(Token::Ident(s)) if s.chars().next().map_or(false, |c| c.is_uppercase()) => {
                    terms.push(Term::Var(s.clone()));
                }
                Some(Token::Ident(s)) => {
                    terms.push(Term::Const(Val::Str(s.clone())));
                }
                Some(Token::StringLiteral(s)) => {
                    terms.push(Term::Const(Val::Str(s.clone())));
                }
                Some(Token::Underscore) => {
                    let name = format!("__anon_{}", self.anon_counter);
                    self.anon_counter += 1;
                    terms.push(Term::Var(name));
                }
                Some(Token::IntLiteral(n)) => {
                    terms.push(Term::Const(Val::Int(*n)));
                }
                Some(t) => return Err(format!("Expected term inside atom, got {:?}", t)),
                None => return Err("Expected term, got end of input".to_string()),
            }

            if self.peek() == Some(&Token::Comma) {
                self.consume(&Token::Comma)?;
            } else if self.peek() == Some(&Token::RParen) {
                break;
            } else {
                return Err("Expected ',' or ')' after term".to_string());
            }
        }
        self.consume(&Token::RParen)?;

        Ok(Atom {
            name,
            terms,
            negated,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Phase 3: Goal-Directed Rule Selection & Semi-Naive Evaluation
// ═══════════════════════════════════════════════════════════════════════════════

/// Backward-chaining: collect predicates that must be materialized for the goals.
/// EDB predicates terminate expansion; only rules on the goal→EDB path are kept.
fn compute_needed_predicates(program: &DatalogProgram, edb: &Edb) -> HashSet<String> {
    let mut needed = HashSet::new();
    let mut pending: Vec<String> = program.goal.iter().map(|a| a.name.clone()).collect();

    let mut rules_by_head: HashMap<String, Vec<&Rule>> = HashMap::new();
    for rule in &program.rules {
        rules_by_head
            .entry(rule.head.name.clone())
            .or_default()
            .push(rule);
    }

    while let Some(pred) = pending.pop() {
        if !needed.insert(pred.clone()) {
            continue;
        }
        if edb.relations.contains_key(&pred) {
            continue;
        }
        if let Some(defining_rules) = rules_by_head.get(&pred) {
            for rule in defining_rules {
                for atom in &rule.body {
                    if !needed.contains(&atom.name) {
                        pending.push(atom.name.clone());
                    }
                }
            }
        }
    }

    needed
}

/// Return only rules whose heads are required to answer the query goals.
fn select_active_rules<'a>(program: &'a DatalogProgram, needed: &HashSet<String>) -> Vec<&'a Rule> {
    program
        .rules
        .iter()
        .filter(|r| needed.contains(&r.head.name))
        .collect()
}

/// The Datalog interpreter: loads EDB, evaluates rules, returns results.
pub struct Interpreter {
    edb: Edb,
    idb: HashMap<String, HashSet<Vec<Val>>>,
    empty: HashSet<Vec<Val>>,
}

impl Interpreter {
    pub fn new(edb: Edb) -> Self {
        Self {
            edb,
            idb: HashMap::new(),
            empty: HashSet::new(),
        }
    }

    /// Get a relation from either EDB or IDB. Returns None for unknown names.
    fn get_relation(&self, name: &str) -> &HashSet<Vec<Val>> {
        if self.idb.contains_key(name) {
            &self.idb[name]
        } else if self.edb.relations.contains_key(name) {
            &self.edb.relations[name]
        } else {
            &self.empty
        }
    }

    /// Run the semi-naive evaluation loop. Returns derived tuples for each goal atom.
    pub fn solve(
        &mut self,
        program: &DatalogProgram,
        step_budget: usize,
    ) -> Result<Vec<(String, Vec<Vec<Val>>)>, String> {
        let start = Instant::now();

        // Seed IDB with explicit program facts
        for fact in &program.facts {
            let mut vals = Vec::new();
            for t in &fact.terms {
                match t {
                    Term::Const(v) => vals.push(v.clone()),
                    _ => return Err("Facts cannot contain variables".to_string()),
                }
            }
            self.idb.entry(fact.name.clone()).or_default().insert(vals);
        }

        let needed = compute_needed_predicates(program, &self.edb);
        let active_rules = select_active_rules(program, &needed);

        if cfg!(test) {
            eprintln!(
                "DEBUG goal-directed: {} active rules of {} (needed: {:?})",
                active_rules.len(),
                program.rules.len(),
                needed
            );
        }

        // Fixed-point iteration — only over goal-reachable rules
        let mut reached_fixpoint = active_rules.is_empty();
        for _step in 0..step_budget {
            if reached_fixpoint {
                break;
            }
            if start.elapsed().as_secs() > 30 {
                return Err("Datalog evaluation timed out (>30s)".to_string());
            }

            let mut changed = false;
            let mut next_idb = self.idb.clone();

            for rule in &active_rules {
                let mut substitutions = vec![HashMap::<String, Val>::new()];

                // Evaluate positive body atoms
                for atom in &rule.body {
                    if atom.negated {
                        continue;
                    }
                    let relation = self.get_relation(&atom.name).clone();
                    let mut next_subs = Vec::new();
                    for sub in &substitutions {
                        for tuple in &relation {
                            if let Some(new_sub) = unify(sub, &atom.terms, tuple) {
                                next_subs.push(new_sub);
                            }
                        }
                    }
                    if next_subs.is_empty() {
                        substitutions.clear();
                        break;
                    }
                    substitutions = next_subs;
                }

                if substitutions.is_empty() {
                    continue;
                }

                // Apply negation filters
                for atom in &rule.body {
                    if !atom.negated {
                        continue;
                    }
                    let relation = self.get_relation(&atom.name);
                    let before = substitutions.len();
                    substitutions.retain(|sub| {
                        let matched = relation
                            .iter()
                            .any(|tuple| unify(sub, &atom.terms, tuple).is_some());
                        if cfg!(test) && matched {
                            eprintln!("    sub {:?} matched edge tuple, filtered out", sub);
                        }
                        !matched
                    });
                    if cfg!(test) {
                        eprintln!(
                            "  negation '{}' filtered {} -> {} subs",
                            atom.name,
                            before,
                            substitutions.len()
                        );
                    }
                }

                // Generate derived tuples
                for sub in &substitutions {
                    let mut head_tuple = Vec::new();
                    for term in &rule.head.terms {
                        match term {
                            Term::Const(v) => head_tuple.push(v.clone()),
                            Term::Var(var) => {
                                if let Some(v) = sub.get(var) {
                                    head_tuple.push(v.clone());
                                }
                            }
                        }
                    }
                    if next_idb
                        .entry(rule.head.name.clone())
                        .or_default()
                        .insert(head_tuple)
                    {
                        changed = true;
                    }
                }
            }

            self.idb = next_idb;
            if !changed {
                reached_fixpoint = true;
                break;
            }
        }

        if !reached_fixpoint {
            return Err(format!(
                "Datalog execution exceeded maximum step limit of {}",
                step_budget
            ));
        }

        // Format goal results
        let mut results = Vec::new();
        for goal_atom in &program.goal {
            let relation = self.get_relation(&goal_atom.name);
            let mut goal_results = Vec::new();

            // If goal has variables, find which positions to project
            let var_positions: Vec<Option<usize>> = goal_atom
                .terms
                .iter()
                .map(|t| match t {
                    Term::Var(v) => Some(v.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .into_iter()
                .enumerate()
                .filter(|(_, v)| v.is_some())
                .map(|(i, _)| Some(i))
                .collect();

            if !var_positions.is_empty() {
                // Project variable columns, respecting constant constraints in the goal
                for tuple in relation {
                    let mut matches = true;
                    for (i, term) in goal_atom.terms.iter().enumerate() {
                        if let Term::Const(expected) = term {
                            if tuple.get(i) != Some(expected) {
                                matches = false;
                                break;
                            }
                        }
                    }
                    if !matches {
                        continue;
                    }
                    let projected: Vec<Val> = var_positions
                        .iter()
                        .filter_map(|&pos| pos.map(|p| tuple.get(p).cloned()).flatten())
                        .collect();
                    if !projected.is_empty() {
                        goal_results.push(projected);
                    }
                }
            } else {
                // No variables: just check if relation is non-empty
                if !relation.is_empty() {
                    goal_results.push(vec![]);
                }
            }

            results.push((goal_atom.name.clone(), goal_results));
        }

        Ok(results)
    }
}

/// Unify a substitution with an atom's terms against a concrete tuple.
/// Returns a new (extended) substitution if unification succeeds.
fn unify(
    sub: &HashMap<String, Val>,
    terms: &[Term],
    tuple: &[Val],
) -> Option<HashMap<String, Val>> {
    if terms.len() != tuple.len() {
        return None;
    }
    let mut new_sub = sub.clone();
    for (term, val) in terms.iter().zip(tuple.iter()) {
        match term {
            Term::Var(var) => match new_sub.get(var) {
                Some(existing) if existing != val => return None,
                _ => {
                    new_sub.insert(var.clone(), val.clone());
                }
            },
            Term::Const(c) if c != val => return None,
            _ => {}
        }
    }
    Some(new_sub)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Phase 4: Safety Guardrails
// ═══════════════════════════════════════════════════════════════════════════════

/// Check whether `from` transitively depends on `to` using the stdlib closure.
pub fn depends_transitive(
    graph: &crate::model::GrapheniumGraph,
    from: &str,
    to: &str,
    step_budget: usize,
) -> Result<bool, String> {
    let query = format!(r#"?- depends_transitive("{from}", "{to}")."#);
    let tokens = tokenize(&query)?;
    let mut program = parse(&tokens)?;
    program.merge_stdlib(stdlib_rules().as_ref().clone());

    let mut edb = Edb::default();
    edb.load_from_graph(graph);
    let mut interpreter = Interpreter::new(edb);
    let results = interpreter.solve(&program, step_budget)?;
    Ok(results.iter().any(|(_, tuples)| !tuples.is_empty()))
}

/// Run a complete Datalog query against a graph with safety guardrails.
pub fn run_datalog_query(
    graph: &crate::model::GrapheniumGraph,
    query: &str,
    step_budget: usize,
) -> Result<String, String> {
    // Guard: empty graph
    if graph.node_count() == 0 {
        return Ok("(empty graph — no nodes to query)".to_string());
    }

    // Guard: query too long
    if query.len() > 10_000 {
        return Err("Query too long (max 10,000 characters)".to_string());
    }

    // Guard: max relation size (prevent OOM)
    let max_relation_size: usize = 100_000;

    // Step 1: Tokenize
    let tokens = tokenize(query)?;

    // Step 2: Parse and merge standard library
    let mut program = parse(&tokens)?;
    program.merge_stdlib(stdlib_rules().as_ref().clone());

    // Step 3: Load EDB
    let mut edb = Edb::default();
    edb.load_from_graph(graph);

    // Guard: check relation sizes
    for (name, facts) in &edb.relations {
        if facts.len() > max_relation_size {
            return Err(format!(
                "Relation '{}' too large ({} facts, max {}). Narrow your query.",
                name,
                facts.len(),
                max_relation_size
            ));
        }
    }

    // Step 4: Evaluate
    let mut interpreter = Interpreter::new(edb);
    let results = interpreter.solve(&program, step_budget)?;

    // Step 5: Format results
    if results.is_empty() || results.iter().all(|(_, tuples)| tuples.is_empty()) {
        return Ok("(no results)".to_string());
    }

    let mut output = String::new();
    for (goal_name, tuples) in results {
        if tuples.is_empty() {
            continue;
        }
        output.push_str(&format!("## Results for `{}`\n\n", goal_name));
        output.push_str("| # | Values |\n|---|---|\n");
        for (i, tuple) in tuples.iter().enumerate() {
            let vals: Vec<String> = tuple.iter().map(|v| format!("{}", v)).collect();
            output.push_str(&format!("| {} | `{}` |\n", i + 1, vals.join(", ")));
        }
        output.push('\n');
    }

    Ok(output)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Phase 6: Verification & Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_test_graph() -> crate::model::GrapheniumGraph {
        let mut g = crate::model::GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "app_ctrl",
            "AppController",
            FileType::Code,
            "src/ctrl.rs",
        ));
        g.upsert_node(Node::new(
            "auth_svc",
            "AuthService",
            FileType::Code,
            "src/auth.rs",
        ));
        g.upsert_node(Node::new(
            "db_conn",
            "DbConnection",
            FileType::Code,
            "src/db.rs",
        ));
        g.add_edge(Edge::extracted(
            "app_ctrl",
            "auth_svc",
            "calls",
            "src/ctrl.rs",
        ));
        g.add_edge(Edge::extracted(
            "auth_svc",
            "db_conn",
            "calls",
            "src/auth.rs",
        ));
        g
    }

    #[test]
    fn test_negation_atom_parsed_correctly() {
        let tokens = tokenize(r#"not edge(X, Y, "calls", _)."#).unwrap();
        let program = parse(&tokens).unwrap();
        assert_eq!(program.facts.len(), 1, "should have 1 fact/rule");
        assert!(
            program.facts[0].negated,
            "atom 'edge' should be negated when preceded by 'not'"
        );
    }

    #[test]
    fn test_bang_negation_atom_parsed_correctly() {
        let tokens = tokenize("!edge(X, Y, \"calls\", _).").unwrap();
        let program = parse(&tokens).unwrap();
        assert_eq!(program.facts.len(), 1, "should have 1 fact/rule");
        assert!(
            program.facts[0].negated,
            "atom 'edge' should be negated when preceded by '!'"
        );
    }

    #[test]
    fn test_tokenize_simple_query() {
        let tokens = tokenize(r#"edge(X, Y, "calls", _)."#).unwrap();
        assert_eq!(tokens.len(), 11);
        assert_eq!(tokens[0], Token::Ident("edge".to_string()));
        assert_eq!(tokens[1], Token::LParen);
        assert!(matches!(tokens[2], Token::Ident(_)));
    }

    #[test]
    fn test_parse_transitive_rule() {
        let input = r#"
            transitive_calls(X, Y) :- edge(X, Y, "calls", _).
            transitive_calls(X, Z) :- edge(X, Y, "calls", _), transitive_calls(Y, Z).
            ?- transitive_calls("app_ctrl", Target).
        "#;
        let tokens = tokenize(input).unwrap();
        let program = parse(&tokens).unwrap();
        assert_eq!(program.rules.len(), 2);
        assert_eq!(program.goal.len(), 1);
    }

    #[test]
    fn test_datalog_program_unification() {
        let graph = make_test_graph();
        let query = r#"
            transitive_calls(X, Y) :- edge(X, Y, "calls", _).
            transitive_calls(X, Z) :- edge(X, Y, "calls", _), transitive_calls(Y, Z).
            ?- transitive_calls("app_ctrl", Target).
        "#;

        let result = run_datalog_query(&graph, query, 1000).unwrap();
        assert!(
            result.contains("auth_svc"),
            "Should find direct call target: {}",
            result
        );
        assert!(
            result.contains("db_conn"),
            "Should find transitive call target: {}",
            result
        );
    }

    #[test]
    fn test_negation_filtering() {
        let graph = make_test_graph();
        let query = r#"
            is_sink(X) :- node(X, _, _, _, _), not calls(X, _, _).
            ?- is_sink(X).
        "#;

        let result = run_datalog_query(&graph, query, 1000).unwrap();
        // Debug: print result
        assert!(
            result.contains("db_conn"),
            "db_conn is a sink but not in result: {}",
            result
        );
        assert!(
            !result.contains("app_ctrl"),
            "app_ctrl is not a sink but appeared: {}",
            result
        );
        assert!(
            !result.contains("auth_svc"),
            "auth_svc is not a sink but appeared: {}",
            result
        );
    }

    #[test]
    fn test_empty_graph_returns_empty() {
        let graph = crate::model::GrapheniumGraph::new();
        let result = run_datalog_query(&graph, "?- node(X, _, _, _, _).", 100).unwrap();
        assert!(
            result.contains("empty graph"),
            "Should indicate empty graph"
        );
    }

    #[test]
    fn test_parse_error_produces_error_message() {
        let graph = make_test_graph();
        let result = run_datalog_query(&graph, "?- invalid syntax!", 100);
        assert!(result.is_err(), "Invalid syntax should produce error");
    }

    #[test]
    fn test_step_limit_exceeded_returns_error() {
        let mut graph = crate::model::GrapheniumGraph::new();
        let nodes = ["n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9"];
        for id in nodes {
            graph.upsert_node(Node::new(id, id, FileType::Code, "chain.rs"));
        }
        for i in 0..nodes.len() - 1 {
            graph.add_edge(Edge::extracted(nodes[i], nodes[i + 1], "calls", "chain.rs"));
        }

        let query = "?- calls_transitive(\"n0\", X).";
        let result = run_datalog_query(&graph, query, 3);
        assert!(result.is_err(), "Should exceed step budget before fixpoint");
        assert!(
            result.unwrap_err().contains("maximum step limit"),
            "Error should mention step limit"
        );
    }

    #[test]
    fn test_is_orphan_negation_filters_connected_nodes() {
        let graph = make_test_graph();
        let mut edb = Edb::default();
        edb.load_from_graph(&graph);
        let mut interpreter = Interpreter::new(edb);

        let program = parse_datalog_program(
            r#"
            is_orphan(X) :- node(X, _, _, _, _), not calls(X, _, _).
            ?- is_orphan(X).
        "#,
        )
        .unwrap();

        let results = interpreter.solve(&program, 100).unwrap();
        let orphans: Vec<_> = results[0]
            .1
            .iter()
            .map(|t| match &t[0] {
                Val::Str(s) => s.clone(),
                _ => String::new(),
            })
            .collect();

        assert!(
            orphans.contains(&"db_conn".to_string()),
            "db_conn has no outgoing calls: {:?}",
            orphans
        );
        assert!(
            !orphans.contains(&"app_ctrl".to_string()),
            "app_ctrl has outgoing calls: {:?}",
            orphans
        );
        assert!(
            !orphans.contains(&"auth_svc".to_string()),
            "auth_svc has outgoing calls: {:?}",
            orphans
        );
    }

    #[test]
    fn test_goal_directed_edb_query_selects_no_rules() {
        let graph = make_test_graph();
        let mut edb = Edb::default();
        edb.load_from_graph(&graph);
        let mut program = parse_datalog_program(r#"?- calls(X, "app_ctrl", _)."#).unwrap();
        program.merge_stdlib(stdlib_rules().as_ref().clone());

        let needed = compute_needed_predicates(&program, &edb);
        assert!(needed.contains("calls"));
        assert!(!needed.contains("calls_transitive"));
        assert!(!needed.contains("depends_transitive"));

        let active = select_active_rules(&program, &needed);
        assert!(active.is_empty(), "EDB-only goals need zero rules");
    }

    #[test]
    fn test_goal_directed_transitive_query_selects_closure_rules_only() {
        let graph = make_test_graph();
        let mut edb = Edb::default();
        edb.load_from_graph(&graph);
        let mut program = parse_datalog_program(r#"?- calls_transitive("app_ctrl", X)."#).unwrap();
        program.merge_stdlib(stdlib_rules().as_ref().clone());

        let needed = compute_needed_predicates(&program, &edb);
        assert!(needed.contains("calls_transitive"));
        assert!(needed.contains("calls"));
        assert!(!needed.contains("depends_transitive"));
        assert!(!needed.contains("is_orphan"));

        let active = select_active_rules(&program, &needed);
        assert!(active.iter().all(|r| r.head.name == "calls_transitive"));
        assert_eq!(active.len(), 2, "two calls_transitive clauses");
    }

    #[test]
    fn test_goal_directed_edb_query_is_instant_on_long_chain() {
        let mut graph = crate::model::GrapheniumGraph::new();
        for i in 0..80 {
            graph.upsert_node(Node::new(
                &format!("n{i}"),
                &format!("N{i}"),
                FileType::Code,
                "chain.rs",
            ));
        }
        for i in 0..79 {
            graph.add_edge(Edge::extracted(
                &format!("n{i}"),
                &format!("n{}", i + 1),
                "calls",
                "chain.rs",
            ));
        }

        let start = Instant::now();
        let result = run_datalog_query(&graph, "?- calls(_, _, _).", 1000).unwrap();
        assert!(
            start.elapsed().as_millis() < 200,
            "EDB query should be instant, took {:?}",
            start.elapsed()
        );
        assert!(!result.contains("no results"), "{result}");
    }

    #[test]
    fn test_self_analysis_graph_edb_queries_are_fast() {
        let path = std::path::Path::new("worked/graphenium-self-analysis/graph.json");
        if !path.exists() {
            return;
        }
        let graph = crate::export::json::load_graph(path).expect("load self-analysis graph");

        let start = Instant::now();
        let hubs = run_datalog_query(&graph, "?- hub(X).", 1000).unwrap();
        assert!(
            start.elapsed().as_secs() < 2,
            "hub/1 EDB query took {:?}",
            start.elapsed()
        );
        assert!(!hubs.contains("no results"), "{hubs}");

        let start = Instant::now();
        let callers = run_datalog_query(
            &graph,
            r#"?- calls(X, "query_run_datalog_query", _)."#,
            1000,
        )
        .unwrap();
        assert!(
            start.elapsed().as_secs() < 2,
            "calls/3 EDB query took {:?}",
            start.elapsed()
        );
        assert!(!callers.contains("no results"), "{callers}");
    }

    #[test]
    fn test_stdlib_embedded_source_parses() {
        let rules = stdlib_rules();
        assert!(!rules.is_empty());
        assert!(rules.iter().any(|r| r.head.name == "calls_transitive"));
        assert!(rules.iter().any(|r| r.head.name == "depends_transitive"));
        assert!(rules.iter().any(|r| r.head.name == "is_hub"));
    }

    #[test]
    fn test_stdlib_transitive_calls_via_run_query() {
        let graph = make_test_graph();
        let result =
            run_datalog_query(&graph, r#"?- calls_transitive("app_ctrl", X)."#, 1000).unwrap();
        assert!(result.contains("auth_svc"), "direct callee: {result}");
        assert!(result.contains("db_conn"), "transitive callee: {result}");
    }

    #[test]
    fn test_edb_loads_typed_relations() {
        let graph = make_test_graph();
        let mut edb = Edb::default();
        edb.load_from_graph(&graph);
        assert!(edb.relations.contains_key("calls"));
        assert!(edb.relations.contains_key("imports"));
        assert!(edb.relations.contains_key("degree"));
        assert_eq!(edb.relations["calls"].len(), 2);
        assert_eq!(edb.relations["node"].len(), 3);
    }

    #[test]
    fn test_edb_loads_node_and_edge() {
        let graph = make_test_graph();
        let mut edb = Edb::default();
        edb.load_from_graph(&graph);
        assert!(edb.relations.contains_key("node"));
        assert!(edb.relations.contains_key("edge"));
        assert_eq!(edb.relations["node"].len(), 3);
        assert_eq!(edb.relations["edge"].len(), 2);
    }
}
