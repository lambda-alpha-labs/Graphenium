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

// ═══════════════════════════════════════════════════════════════════════════════
// Phase 1: EDB Relational Loader — data types and extensional database
// ═══════════════════════════════════════════════════════════════════════════════

/// A single value in a relation tuple.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub enum Val {
    Str(Arc<str>),
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

impl Atom {
    /// `true` when this atom is an internal inequality constraint.
    pub fn is_inequality(&self) -> bool {
        self.name == INEQUALITY_PREDICATE
    }
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

/// Intern a string so every tuple shares the same `Arc<str>` allocation.
fn intern_arc(interned: &mut HashMap<String, Arc<str>>, value: &str) -> Arc<str> {
    if let Some(existing) = interned.get(value) {
        return existing.clone();
    }
    let arc: Arc<str> = Arc::from(value);
    interned.insert(value.to_string(), arc.clone());
    arc
}

/// Names of every EDB relation materialized from a graph.
pub fn edb_relation_names() -> HashSet<String> {
    [
        "node",
        "calls",
        "imports",
        "contains",
        "inherits",
        "implements",
        "edge",
        "degree",
        "hub",
    ]
    .iter()
    .map(|name| (*name).to_string())
    .collect()
}

impl Edb {
    /// Load every fact directly from a GrapheniumGraph.
    pub fn load_from_graph(&mut self, graph: &crate::model::GrapheniumGraph) {
        let all = edb_relation_names();
        self.load_from_graph_selective(graph, &all);
    }

    /// Load only the EDB relations in `include`. Goal-directed evaluation uses
    /// this to avoid materializing multi-hundred-thousand-row relations that no
    /// active rule can reach.
    pub fn load_from_graph_selective(
        &mut self,
        graph: &crate::model::GrapheniumGraph,
        include: &HashSet<String>,
    ) {
        if cfg!(test) {
            eprintln!(
                "DEBUG load_from_graph: {} nodes, {} edges",
                graph.node_count(),
                graph.edges_iter().count()
            );
        }
        let mut interned: HashMap<String, Arc<str>> = HashMap::new();
        let want = |name: &str| include.contains(name);
        self.relations.clear();

        // 1. node(Id, Label, Type, File, Community)
        if want("node") {
            let mut nodes = HashSet::new();
            for n in graph.nodes() {
                nodes.insert(vec![
                    Val::Str(intern_arc(&mut interned, &n.id)),
                    Val::Str(intern_arc(&mut interned, &n.label)),
                    Val::Str(intern_arc(&mut interned, &format!("{:?}", n.file_type))),
                    Val::Str(intern_arc(&mut interned, &n.source_file)),
                    Val::Int(n.community.unwrap_or(999) as i64),
                ]);
            }
            self.relations.insert("node".to_string(), nodes);
        }

        // 2. Typed relations: calls, imports, contains, inherits, implements
        let typed = ["calls", "imports", "contains", "inherits", "implements"];
        let want_edge = want("edge");
        let want_any_typed = typed.iter().any(|name| want(name));
        let mut calls = HashSet::new();
        let mut imports = HashSet::new();
        let mut contains = HashSet::new();
        let mut inherits = HashSet::new();
        let mut implements = HashSet::new();
        let mut edges = HashSet::new();
        if want_edge || want_any_typed {
            for e in graph.edges_iter() {
                let confidence = match e.confidence {
                    crate::model::Confidence::Extracted => "EXTRACTED",
                    crate::model::Confidence::Inferred => "INFERRED",
                    crate::model::Confidence::Ambiguous => "AMBIGUOUS",
                };
                let source = intern_arc(&mut interned, &e.source);
                let target = intern_arc(&mut interned, &e.target);
                if want_any_typed && typed.contains(&e.relation.as_str()) {
                    let tuple = vec![
                        Val::Str(source.clone()),
                        Val::Str(target.clone()),
                        Val::Str(intern_arc(&mut interned, confidence)),
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
                }
                if want_edge {
                    edges.insert(vec![
                        Val::Str(source),
                        Val::Str(target),
                        Val::Str(intern_arc(&mut interned, &e.relation)),
                        Val::Str(intern_arc(&mut interned, confidence)),
                    ]);
                }
            }
        }
        if want("calls") {
            self.relations.insert("calls".to_string(), calls);
        }
        if want("imports") {
            self.relations.insert("imports".to_string(), imports);
        }
        if want("contains") {
            self.relations.insert("contains".to_string(), contains);
        }
        if want("inherits") {
            self.relations.insert("inherits".to_string(), inherits);
        }
        if want("implements") {
            self.relations.insert("implements".to_string(), implements);
        }
        if want_edge {
            self.relations.insert("edge".to_string(), edges);
        }

        // 3. degree(NodeId, Count) and hub(NodeId) for is_hub/1
        if want("degree") || want("hub") {
            let mut degrees = HashSet::new();
            let mut hubs = HashSet::new();
            for n in graph.nodes() {
                let count = graph.degree(&n.id) as i64;
                let id = intern_arc(&mut interned, &n.id);
                if want("degree") {
                    degrees.insert(vec![Val::Str(id.clone()), Val::Int(count)]);
                }
                if want("hub") && count > 15 {
                    hubs.insert(vec![Val::Str(id)]);
                }
            }
            if want("degree") {
                self.relations.insert("degree".to_string(), degrees);
            }
            if want("hub") {
                self.relations.insert("hub".to_string(), hubs);
            }
        }
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
    LBracket,   // [
    RBracket,   // ]
    Dot,        // .
    Not,        // !
    NotEqual,   // != or /=
    Underscore, // _
}

/// Predicate name used internally to represent an inequality constraint
/// (`A != B` / `A /= B`) inside a rule or query body.
pub const INEQUALITY_PREDICATE: &str = "__inequality__";

/// A labelled (or anonymous) `?-` query in a Datalog program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DatalogQuery {
    /// Optional `[name]` label, e.g. `?- [gaps] has_proof_gap(X).`
    pub name: Option<String>,
    /// Conjunctive goals. Inequality constraints are encoded as atoms whose
    /// name is [`INEQUALITY_PREDICATE`] so the existing `Atom` shape (and all
    /// downstream consumers) stays unchanged.
    pub goals: Vec<Atom>,
}

/// A parsed Datalog program.
///
/// `goal` is kept as a backwards-compatible alias for the first query's goals.
/// New code should use `queries`, which supports multiple `?-` blocks.
#[derive(Default, Debug)]
pub struct DatalogProgram {
    pub facts: Vec<Atom>,
    pub rules: Vec<Rule>,
    pub goal: Vec<Atom>,
    pub queries: Vec<DatalogQuery>,
}

impl DatalogProgram {
    /// Merge standard library rules into the parsed program.
    /// Stdlib rules are prepended so they are evaluated before user rules.
    pub fn merge_stdlib(&mut self, stdlib_rules: Vec<Rule>) {
        let mut combined = stdlib_rules;
        combined.append(&mut self.rules);
        self.rules = combined;
    }

    /// Keep the legacy `goal` field in sync with the first query.
    fn sync_legacy_goal(&mut self) {
        self.goal = self
            .queries
            .first()
            .map(|query| query.goals.clone())
            .unwrap_or_default();
    }

    /// Every goal atom across all `?-` queries.
    pub fn all_goals(&self) -> impl Iterator<Item = &Atom> {
        self.queries.iter().flat_map(|query| query.goals.iter())
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
            '[' => {
                tokens.push(Token::LBracket);
                i += 1;
            }
            ']' => {
                tokens.push(Token::RBracket);
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
            '!' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::NotEqual);
                i += 2;
            }
            '!' => {
                tokens.push(Token::Not);
                i += 1;
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::NotEqual);
                i += 2;
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

                // Optional `[name]` label: `?- [proof_gaps] has_proof_gap(X).`
                let name = if self.peek() == Some(&Token::LBracket) {
                    self.consume(&Token::LBracket)?;
                    let label = match self.next() {
                        Some(Token::Ident(s)) | Some(Token::StringLiteral(s)) => s.clone(),
                        Some(t) => {
                            return Err(format!("Expected query label inside [...], got {:?}", t))
                        }
                        None => return Err("Expected query label, got end of input".to_string()),
                    };
                    self.consume(&Token::RBracket)?;
                    Some(label)
                } else {
                    None
                };

                let mut goals = Vec::new();
                if self.peek() != Some(&Token::Dot) {
                    loop {
                        goals.push(self.parse_body_item()?);
                        if self.peek() == Some(&Token::Comma) {
                            self.consume(&Token::Comma)?;
                        } else {
                            break;
                        }
                    }
                }
                self.consume(&Token::Dot)?;
                program.queries.push(DatalogQuery { name, goals });
                program.sync_legacy_goal();
            } else {
                let atom = self.parse_atom()?;
                if self.peek() == Some(&Token::Arrow) {
                    self.consume(&Token::Arrow)?;
                    let mut body = Vec::new();
                    loop {
                        body.push(self.parse_body_item()?);
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

    /// Parse one body/goal item: a (possibly negated) relation atom, or an
    /// infix inequality `Term != Term` / `Term /= Term`.
    fn parse_body_item(&mut self) -> Result<Atom, String> {
        let negated_start = matches!(self.peek(), Some(Token::Not))
            || matches!(self.peek(), Some(Token::Ident(s)) if s == "not");

        // A relation atom always looks like `ident(...)`, so only an
        // identifier followed by `(` (or an explicit `not`/`!`) can start one.
        let starts_relation = negated_start
            || matches!(self.peek(), Some(Token::Ident(_)))
                && matches!(self.tokens.get(self.pos + 1), Some(Token::LParen));

        if starts_relation {
            return self.parse_atom();
        }

        let left = self.parse_term()?;
        match self.next() {
            Some(Token::NotEqual) => {}
            Some(t) => {
                return Err(format!(
                    "Expected '!=' or '/=' after term in body, got {:?}",
                    t
                ))
            }
            None => return Err("Expected '!=' or '/=' after term, got end of input".to_string()),
        }
        let right = self.parse_term()?;
        Ok(Atom {
            name: INEQUALITY_PREDICATE.to_string(),
            terms: vec![left, right],
            negated: false,
        })
    }

    fn parse_term(&mut self) -> Result<Term, String> {
        match self.next() {
            Some(Token::Ident(s)) if s.chars().next().is_some_and(|c| c.is_uppercase()) => {
                Ok(Term::Var(s.clone()))
            }
            Some(Token::Ident(s)) => Ok(Term::Const(Val::Str(s.clone().into()))),
            Some(Token::StringLiteral(s)) => Ok(Term::Const(Val::Str(s.clone().into()))),
            Some(Token::Underscore) => {
                let name = format!("__anon_{}", self.anon_counter);
                self.anon_counter += 1;
                Ok(Term::Var(name))
            }
            Some(Token::IntLiteral(n)) => Ok(Term::Const(Val::Int(*n))),
            Some(t) => Err(format!("Expected term, got {:?}", t)),
            None => Err("Expected term, got end of input".to_string()),
        }
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
                Some(Token::Ident(s)) if s.chars().next().is_some_and(|c| c.is_uppercase()) => {
                    terms.push(Term::Var(s.clone()));
                }
                Some(Token::Ident(s)) => {
                    terms.push(Term::Const(Val::Str(s.clone().into())));
                }
                Some(Token::StringLiteral(s)) => {
                    terms.push(Term::Const(Val::Str(s.clone().into())));
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
    compute_needed_predicates_with(program, edb.relations.keys())
}

/// Goal-directed predicate collection against a set of available EDB names.
fn compute_needed_predicates_with<'a>(
    program: &DatalogProgram,
    edb_names: impl IntoIterator<Item = &'a String>,
) -> HashSet<String> {
    let edb_names: HashSet<&str> = edb_names.into_iter().map(String::as_str).collect();
    let mut needed = HashSet::new();
    let mut pending: Vec<String> = program
        .all_goals()
        .filter(|a| !a.is_inequality())
        .map(|a| a.name.clone())
        .collect();

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
        if edb_names.contains(pred.as_str()) {
            continue;
        }
        if let Some(defining_rules) = rules_by_head.get(&pred) {
            for rule in defining_rules {
                for atom in &rule.body {
                    if atom.is_inequality() {
                        continue;
                    }
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

/// Intern `name` in the predicate table, returning its dense index.
fn intern_predicate(
    name: &str,
    index: &mut HashMap<String, usize>,
    names: &mut Vec<String>,
) -> usize {
    if let Some(&id) = index.get(name) {
        return id;
    }
    let id = names.len();
    names.push(name.to_string());
    index.insert(name.to_string(), id);
    id
}

/// Tarjan state for strongly-connected-component discovery.
struct TarjanState<'a> {
    adj: &'a [Vec<usize>],
    next_index: usize,
    indices: Vec<Option<usize>>,
    low: Vec<usize>,
    stack: Vec<usize>,
    on_stack: Vec<bool>,
    component: Vec<usize>,
    component_count: usize,
}

impl<'a> TarjanState<'a> {
    fn new(adj: &'a [Vec<usize>]) -> Self {
        let n = adj.len();
        Self {
            adj,
            next_index: 0,
            indices: vec![None; n],
            low: vec![0; n],
            stack: Vec::new(),
            on_stack: vec![false; n],
            component: vec![usize::MAX; n],
            component_count: 0,
        }
    }

    fn strongconnect(&mut self, node: usize) {
        let index = self.next_index;
        self.next_index += 1;
        self.indices[node] = Some(index);
        self.low[node] = index;
        self.stack.push(node);
        self.on_stack[node] = true;

        let out_edges = self.adj[node].clone();
        for next in out_edges {
            if self.indices[next].is_none() {
                self.strongconnect(next);
                self.low[node] = self.low[node].min(self.low[next]);
            } else if self.on_stack[next] {
                if let Some(next_index) = self.indices[next] {
                    self.low[node] = self.low[node].min(next_index);
                }
            }
        }

        if self.low[node] == self.indices[node].unwrap_or(usize::MAX) {
            let component = self.component_count;
            self.component_count += 1;
            while let Some(member) = self.stack.pop() {
                self.on_stack[member] = false;
                self.component[member] = component;
                if member == node {
                    break;
                }
            }
        }
    }
}

/// Partition active rules into strata using the predicate dependency graph.
///
/// Positive body atoms contribute a non-strict dependency edge (`head` depends
/// on `body`); negated atoms contribute a strict edge. Strongly-connected
/// components are computed first, then strata are assigned over the
/// condensation DAG. A negative edge inside one SCC is unstratifiable and is
/// rejected instead of being evaluated to a non-terminating fixpoint.
fn plan_strata<'a>(active_rules: &[&'a Rule]) -> Result<Vec<Vec<&'a Rule>>, String> {
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut edges: Vec<(usize, usize, bool)> = Vec::new();

    for rule in active_rules {
        let head = intern_predicate(&rule.head.name, &mut index, &mut names);
        for atom in &rule.body {
            if atom.is_inequality() {
                continue;
            }
            let body = intern_predicate(&atom.name, &mut index, &mut names);
            edges.push((head, body, atom.negated));
        }
    }

    let n = names.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (head, body, _) in &edges {
        adj[*head].push(*body);
    }

    let mut tarjan = TarjanState::new(&adj);
    for node in 0..n {
        if tarjan.indices[node].is_none() {
            tarjan.strongconnect(node);
        }
    }
    let component = tarjan.component;
    let component_count = tarjan.component_count;

    // A cycle containing a negated edge has no stratification.
    for (head, body, negated) in &edges {
        if *negated && component[*head] == component[*body] {
            return Err(format!(
                "unstratifiable negation: predicate `{}` occurs in a recursive cycle \
                 that contains a negated dependency on `{}`; break the cycle or remove the \
                 negation",
                names[*head], names[*body]
            ));
        }
    }

    // Assign each component a stratum. Positive dependencies allow equality;
    // negative dependencies require at least one extra stratum. The
    // condensation graph is acyclic, so this relaxation terminates.
    let mut component_stratum = vec![0usize; component_count];
    let mut iterations = 0usize;
    loop {
        let mut changed = false;
        iterations += 1;
        if iterations > component_count + 1 {
            return Err(
                "unstratifiable negation: predicate dependency graph did not converge".to_string(),
            );
        }
        for (head, body, negated) in &edges {
            let head_component = component[*head];
            let body_component = component[*body];
            if head_component == body_component {
                continue;
            }
            let required = component_stratum[body_component] + usize::from(*negated);
            if component_stratum[head_component] < required {
                component_stratum[head_component] = required;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let max_stratum = component_stratum.iter().copied().max().unwrap_or(0);
    let mut strata: Vec<Vec<&'a Rule>> = vec![Vec::new(); max_stratum + 1];
    for rule in active_rules {
        let head = index[&rule.head.name];
        let stratum = component_stratum[component[head]];
        strata[stratum].push(*rule);
    }
    Ok(strata)
}

/// Frozen relation snapshot with a full-tuple hash set and bound-mask hash
/// indexes. Frozen relations belong to already-evaluated strata (or the EDB), so
/// the snapshot cannot become stale while a higher stratum is running.
///
/// All-ground negated atoms use [`RelationIndex::contains_exact`] in `O(1)`.
/// Partially bound atoms use an index keyed by the exact set of bound columns,
/// so negation never falls back to a linear scan regardless of which column
/// (leading or not) participates in the join.
#[derive(Debug, Clone, Default)]
struct RelationIndex {
    tuples: Vec<Vec<Val>>,
    full: HashSet<Vec<Val>>,
    masks: HashMap<usize, HashMap<Vec<Val>, Vec<usize>>>,
    arity: usize,
}

impl RelationIndex {
    fn new(tuples: Vec<Vec<Val>>) -> Self {
        let arity = tuples.iter().map(Vec::len).max().unwrap_or(0);
        let full = tuples.iter().cloned().collect();
        Self {
            tuples,
            full,
            masks: HashMap::new(),
            arity,
        }
    }

    /// Build the index for one bound-column mask on first use.
    fn ensure_mask(&mut self, mask: usize) {
        if mask == 0 || self.arity == 0 {
            return;
        }
        let full_mask = self.full_mask();
        if mask == full_mask || self.masks.contains_key(&mask) {
            return;
        }
        let mut index: HashMap<Vec<Val>, Vec<usize>> = HashMap::new();
        for (row, tuple) in self.tuples.iter().enumerate() {
            let key = mask_key(tuple, mask);
            index.entry(key).or_default().push(row);
        }
        self.masks.insert(mask, index);
    }

    fn full_mask(&self) -> usize {
        if self.arity >= usize::BITS as usize {
            usize::MAX
        } else {
            (1usize << self.arity) - 1
        }
    }

    fn contains_exact(&self, tuple: &[Val]) -> bool {
        self.full.contains(tuple)
    }

    /// `true` when any indexed tuple matches the compiled positive/negated
    /// atom under the current variable bindings.
    fn matches_compiled(&self, terms: &[CompiledTerm<'_>], bindings: &[Option<Val>]) -> bool {
        if self.tuples.is_empty() {
            return false;
        }
        if terms.len() != self.tuples[0].len() {
            return false;
        }

        let mut bound_values: Vec<Option<&Val>> = Vec::with_capacity(terms.len());
        let mut all_bound = true;
        let mut mask = 0usize;
        for (column, term) in terms.iter().enumerate() {
            let value = match term {
                CompiledTerm::Const(value) => Some(*value),
                CompiledTerm::Var(index) => bindings.get(*index).and_then(Option::as_ref),
            };
            if value.is_some() {
                mask |= 1usize << column;
            } else {
                all_bound = false;
            }
            bound_values.push(value);
        }

        let Some(key) = bound_values_key(&bound_values) else {
            // Nothing is bound: existence of any tuple is enough.
            return !self.tuples.is_empty();
        };

        if all_bound {
            return self.contains_exact(&key);
        }

        if let Some(index) = self.masks.get(&mask) {
            return index.get(&key).is_some_and(|rows| {
                rows.iter()
                    .any(|&row| row_matches_bindings(terms, &self.tuples[row], bindings))
            });
        }

        // The evaluation planner normally pre-builds every required mask; this
        // fallback preserves correctness if a caller constructs an index
        // directly.
        self.tuples
            .iter()
            .any(|tuple| row_matches_bindings(terms, tuple, bindings))
    }
}

fn bound_values_key(bound_values: &[Option<&Val>]) -> Option<Vec<Val>> {
    let key: Vec<Val> = bound_values
        .iter()
        .filter_map(|value| value.cloned())
        .collect();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

fn mask_key(tuple: &[Val], mask: usize) -> Vec<Val> {
    tuple
        .iter()
        .enumerate()
        .filter(|(column, _)| mask & (1usize << column) != 0)
        .map(|(_, value)| value.clone())
        .collect()
}

/// A term compiled against the body's variable table.
#[derive(Clone, Copy)]
enum CompiledTerm<'a> {
    Var(usize),
    Const(&'a Val),
}

struct CompiledAtom<'a> {
    body_index: usize,
    name: &'a str,
    terms: Vec<CompiledTerm<'a>>,
}

struct CompiledBody<'a> {
    positive: Vec<CompiledAtom<'a>>,
    filters: Vec<CompiledAtom<'a>>,
    inequalities: Vec<(CompiledTerm<'a>, CompiledTerm<'a>)>,
}

fn compile_term<'a>(term: &'a Term, variables: &mut HashMap<&'a str, usize>) -> CompiledTerm<'a> {
    match term {
        Term::Const(value) => CompiledTerm::Const(value),
        Term::Var(name) => {
            let next = variables.len();
            let index = *variables.entry(name.as_str()).or_insert(next);
            CompiledTerm::Var(index)
        }
    }
}

fn compile_body<'a>(
    body: &'a [Atom],
    head_terms: &'a [Term],
    variables: &mut HashMap<&'a str, usize>,
) -> CompiledBody<'a> {
    let mut positive = Vec::new();
    let mut filters = Vec::new();
    let mut inequalities = Vec::new();

    for (body_index, atom) in body.iter().enumerate() {
        if atom.is_inequality() {
            if let (Some(left), Some(right)) = (atom.terms.first(), atom.terms.get(1)) {
                inequalities.push((
                    compile_term(left, variables),
                    compile_term(right, variables),
                ));
            }
            continue;
        }
        let compiled = CompiledAtom {
            body_index,
            name: atom.name.as_str(),
            terms: atom
                .terms
                .iter()
                .map(|term| compile_term(term, variables))
                .collect(),
        };
        if atom.negated {
            filters.push(compiled);
        } else {
            positive.push(compiled);
        }
    }

    // Ensure head variables receive indices even if they do not occur in the
    // body (an unsafe rule would leave them unbound and drop the row).
    for term in head_terms {
        compile_term(term, variables);
    }

    CompiledBody {
        positive,
        filters,
        inequalities,
    }
}

/// Non-allocating match check used by indices.
fn row_matches_bindings(
    terms: &[CompiledTerm<'_>],
    tuple: &[Val],
    bindings: &[Option<Val>],
) -> bool {
    if terms.len() != tuple.len() {
        return false;
    }
    terms
        .iter()
        .zip(tuple.iter())
        .all(|(term, value)| match term {
            CompiledTerm::Var(index) => bindings
                .get(*index)
                .and_then(Option::as_ref)
                .is_none_or(|existing| existing == value),
            CompiledTerm::Const(constant) => *constant == value,
        })
}

/// Bind one tuple against an atom's compiled terms. Newly bound variable
/// indices are appended to `undo` so the caller can roll back.
fn bind_terms(
    terms: &[CompiledTerm<'_>],
    tuple: &[Val],
    bindings: &mut [Option<Val>],
    undo: &mut Vec<usize>,
) -> bool {
    if terms.len() != tuple.len() {
        return false;
    }
    for (term, value) in terms.iter().zip(tuple.iter()) {
        match term {
            CompiledTerm::Const(constant) => {
                if *constant != value {
                    return false;
                }
            }
            CompiledTerm::Var(index) => match bindings.get(*index) {
                Some(Some(existing)) if existing != value => return false,
                Some(Some(_)) => {}
                _ => {
                    if let Some(slot) = bindings.get_mut(*index) {
                        *slot = Some(value.clone());
                        undo.push(*index);
                    } else {
                        return false;
                    }
                }
            },
        }
    }
    true
}

/// Backtracking join over positive atoms, with inequalities and negated atoms
/// applied only after every positive variable is bound.
#[allow(clippy::too_many_arguments)]
fn join_atoms(
    atom_index: usize,
    positive: &[CompiledAtom<'_>],
    relations: &[&HashSet<Vec<Val>>],
    filters: &[CompiledAtom<'_>],
    filter_indexes: &[&RelationIndex],
    inequalities: &[(CompiledTerm<'_>, CompiledTerm<'_>)],
    head: &[CompiledTerm<'_>],
    bindings: &mut [Option<Val>],
    undo: &mut Vec<usize>,
    rows: &mut Vec<Vec<Val>>,
) {
    if atom_index == positive.len() {
        for (filter, index) in filters.iter().zip(filter_indexes.iter()) {
            if index.matches_compiled(&filter.terms, bindings) {
                return;
            }
        }
        for (left, right) in inequalities {
            let left = resolve_compiled(left, bindings);
            let right = resolve_compiled(right, bindings);
            match (left, right) {
                (Some(left), Some(right)) if left != right => {}
                _ => return,
            }
        }

        let mut row = Vec::with_capacity(head.len());
        for term in head {
            match term {
                CompiledTerm::Const(value) => row.push((*value).clone()),
                CompiledTerm::Var(index) => match bindings.get(*index).and_then(Option::as_ref) {
                    Some(value) => row.push(value.clone()),
                    // Unsafe head variable: match the historical behaviour of
                    // dropping the incomplete row.
                    None => return,
                },
            }
        }
        rows.push(row);
        return;
    }

    let atom = &positive[atom_index];
    for tuple in relations[atom_index].iter() {
        let mark = undo.len();
        if bind_terms(&atom.terms, tuple, bindings, undo) {
            join_atoms(
                atom_index + 1,
                positive,
                relations,
                filters,
                filter_indexes,
                inequalities,
                head,
                bindings,
                undo,
                rows,
            );
        }
        while undo.len() > mark {
            if let Some(index) = undo.pop() {
                bindings[index] = None;
            }
        }
    }
}

fn resolve_compiled<'a>(term: &CompiledTerm<'a>, bindings: &'a [Option<Val>]) -> Option<&'a Val> {
    match term {
        CompiledTerm::Const(value) => Some(*value),
        CompiledTerm::Var(index) => bindings.get(*index).and_then(Option::as_ref),
    }
}

/// The Datalog interpreter: loads EDB, evaluates rules, returns results.
pub struct Interpreter {
    edb: Edb,
    idb: HashMap<String, HashSet<Vec<Val>>>,
    empty: HashSet<Vec<Val>>,
    /// Lazily-built indices for relations in frozen (lower) strata.
    negation_indexes: HashMap<String, RelationIndex>,
}

impl Interpreter {
    pub fn new(edb: Edb) -> Self {
        Self {
            edb,
            idb: HashMap::new(),
            empty: HashSet::new(),
            negation_indexes: HashMap::new(),
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

    /// True when every goal atom in `goals` is fully ground and already present
    /// in the IDB/EDB. Inequality constraints are evaluated against the goal's
    /// own constants and therefore cannot be checked without a substitution.
    fn goals_satisfied(&self, goals: &[Atom]) -> bool {
        if goals.is_empty() {
            return false;
        }
        goals.iter().all(|atom| {
            if atom.negated || atom.is_inequality() {
                return false;
            }
            let mut tuple = Vec::with_capacity(atom.terms.len());
            for term in &atom.terms {
                match term {
                    Term::Const(v) => tuple.push(v.clone()),
                    Term::Var(_) => return false,
                }
            }
            self.get_relation(&atom.name).contains(&tuple)
        })
    }

    /// Every `?-` query must be fully satisfied before the monotone loop may
    /// stop early.
    fn all_queries_satisfied(&self, program: &DatalogProgram) -> bool {
        !program.queries.is_empty()
            && program
                .queries
                .iter()
                .all(|query| self.goals_satisfied(&query.goals))
    }

    /// Evaluate a body (rule body or query goals) and derive rows for
    /// `head_terms`. Runs a backtracking tuple join with a single reusable
    /// binding table, so a 200k-row scan does not allocate a `HashMap` per
    /// intermediate substitution.
    fn evaluate_rows(
        &mut self,
        body: &[Atom],
        head_terms: &[Term],
        delta_pos: Option<usize>,
        delta: &HashMap<String, HashSet<Vec<Val>>>,
    ) -> Vec<Vec<Val>> {
        let mut variables: HashMap<&str, usize> = HashMap::new();
        let compiled = compile_body(body, head_terms, &mut variables);
        let head: Vec<CompiledTerm<'_>> = head_terms
            .iter()
            .map(|term| compile_term(term, &mut variables))
            .collect();

        // Materialize every negated-relation index before taking the immutable
        // relation borrows below. Pre-build exactly the bound-column masks used
        // by these filters (not every possible subset), keeping memory bounded
        // while still guaranteeing O(1) lookups for non-leading columns.
        let mut positive_vars: HashSet<usize> = HashSet::new();
        for atom in &compiled.positive {
            for term in &atom.terms {
                if let CompiledTerm::Var(index) = term {
                    positive_vars.insert(*index);
                }
            }
        }
        let mut required_masks: HashMap<&str, HashSet<usize>> = HashMap::new();
        for filter in &compiled.filters {
            let mut mask = 0usize;
            for (column, term) in filter.terms.iter().enumerate() {
                let bound = match term {
                    CompiledTerm::Const(_) => true,
                    CompiledTerm::Var(index) => positive_vars.contains(index),
                };
                if bound {
                    mask |= 1usize << column;
                }
            }
            required_masks.entry(filter.name).or_default().insert(mask);
        }
        for (name, masks) in required_masks {
            if !self.negation_indexes.contains_key(name) {
                let tuples: Vec<Vec<Val>> = self.get_relation(name).iter().cloned().collect();
                self.negation_indexes
                    .insert(name.to_string(), RelationIndex::new(tuples));
            }
            if let Some(index) = self.negation_indexes.get_mut(name) {
                for mask in masks {
                    index.ensure_mask(mask);
                }
            }
        }
        let filter_indexes: Vec<&RelationIndex> = compiled
            .filters
            .iter()
            .map(|filter| &self.negation_indexes[filter.name])
            .collect();

        let relations: Vec<&HashSet<Vec<Val>>> = compiled
            .positive
            .iter()
            .map(|atom| {
                if delta_pos == Some(atom.body_index) {
                    delta.get(atom.name).unwrap_or(&self.empty)
                } else {
                    self.get_relation(atom.name)
                }
            })
            .collect();

        let mut bindings: Vec<Option<Val>> = vec![None; variables.len()];
        let mut undo: Vec<usize> = Vec::new();
        let mut rows = Vec::new();
        join_atoms(
            0,
            &compiled.positive,
            &relations,
            &compiled.filters,
            &filter_indexes,
            &compiled.inequalities,
            &head,
            &mut bindings,
            &mut undo,
            &mut rows,
        );
        rows
    }

    /// Evaluate a single rule body and derive head tuples.
    fn evaluate_rule(
        &mut self,
        rule: &Rule,
        delta_pos: Option<usize>,
        delta: &HashMap<String, HashSet<Vec<Val>>>,
    ) -> Vec<Vec<Val>> {
        self.evaluate_rows(&rule.body, &rule.head.terms, delta_pos, delta)
    }

    /// Evaluate one conjunctive query and project its named variables.
    /// Returns `(variables, rows)`, with duplicate rows removed.
    fn evaluate_query(&mut self, goals: &[Atom]) -> (Vec<String>, Vec<Vec<Val>>) {
        let variables = query_variables(goals);
        let head: Vec<Term> = variables
            .iter()
            .map(|name| Term::Var(name.clone()))
            .collect();
        let empty_delta = HashMap::new();
        let mut rows = self.evaluate_rows(goals, &head, None, &empty_delta);
        rows.sort();
        rows.dedup();
        (variables, rows)
    }

    /// Run the semi-naive evaluation loop. Returns `(query_name, rows)` for
    /// every `?-` query.
    pub fn solve(
        &mut self,
        program: &DatalogProgram,
        step_budget: usize,
    ) -> Result<Vec<(String, Vec<Vec<Val>>)>, String> {
        self.solve_with_steps(program, step_budget)
            .map(|(results, _steps)| results)
    }

    /// Like [`Interpreter::solve`], but also returns the number of fixed-point
    /// iterations actually consumed.
    pub fn solve_with_steps(
        &mut self,
        program: &DatalogProgram,
        step_budget: usize,
    ) -> Result<(Vec<(String, Vec<Vec<Val>>)>, usize), String> {
        self.solve_internal(program, step_budget, false)
            .map(|(results, steps, _)| (results, steps))
    }

    /// Like [`Interpreter::solve_with_steps`], but also returns a snapshot of
    /// every materialized IDB relation. Used by `--show-derived`.
    pub fn solve_with_steps_and_idb(
        &mut self,
        program: &DatalogProgram,
        step_budget: usize,
    ) -> Result<
        (
            Vec<(String, Vec<Vec<Val>>)>,
            usize,
            HashMap<String, Vec<Vec<Val>>>,
        ),
        String,
    > {
        self.solve_internal(program, step_budget, true)
            .map(|(results, steps, idb)| (results, steps, idb.unwrap_or_default()))
    }

    fn solve_internal(
        &mut self,
        program: &DatalogProgram,
        step_budget: usize,
        capture_idb: bool,
    ) -> Result<
        (
            Vec<(String, Vec<Vec<Val>>)>,
            usize,
            Option<HashMap<String, Vec<Vec<Val>>>>,
        ),
        String,
    > {
        let start = Instant::now();

        validate_grounding(program)?;

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

        // Partition goal-reachable rules into strata. Lower strata are fully
        // materialized before any higher stratum that negates them runs, so
        // every `not p(...)` sees a frozen relation.
        let strata = plan_strata(&active_rules)?;
        let mut steps_used = 0usize;
        let mut queries_done = self.all_queries_satisfied(program);

        for rules in &strata {
            if queries_done {
                break;
            }
            let idb_predicates: HashSet<String> =
                rules.iter().map(|rule| rule.head.name.clone()).collect();

            // Seed the delta with explicit facts / any tuples already derived
            // for this stratum so rules that consume them fire.
            let mut delta: HashMap<String, HashSet<Vec<Val>>> = HashMap::new();
            for predicate in &idb_predicates {
                if let Some(tuples) = self.idb.get(predicate) {
                    if !tuples.is_empty() {
                        delta.insert(predicate.clone(), tuples.clone());
                    }
                }
            }

            let mut first_iteration = true;
            loop {
                if start.elapsed().as_secs() > 30 {
                    return Err("Datalog evaluation timed out (>30s)".to_string());
                }
                if steps_used >= step_budget {
                    return Err(format!(
                        "Datalog execution exceeded maximum step limit of {step_budget}"
                    ));
                }
                steps_used += 1;

                let mut derived: HashMap<String, HashSet<Vec<Val>>> = HashMap::new();

                for rule in rules {
                    let positive_idb_positions: Vec<usize> = rule
                        .body
                        .iter()
                        .enumerate()
                        .filter(|(_, atom)| {
                            !atom.negated
                                && !atom.is_inequality()
                                && idb_predicates.contains(&atom.name)
                        })
                        .map(|(idx, _)| idx)
                        .collect();

                    if positive_idb_positions.is_empty() {
                        // Base rule: its inputs are EDB or lower-stratum
                        // relations, so evaluating it once per stratum is
                        // sufficient.
                        if !first_iteration {
                            continue;
                        }
                        for tuple in self.evaluate_rule(rule, None, &delta) {
                            derived
                                .entry(rule.head.name.clone())
                                .or_default()
                                .insert(tuple);
                        }
                        continue;
                    }

                    for pos in positive_idb_positions {
                        for tuple in self.evaluate_rule(rule, Some(pos), &delta) {
                            derived
                                .entry(rule.head.name.clone())
                                .or_default()
                                .insert(tuple);
                        }
                    }
                }

                // Merge newly derived tuples into the IDB and build the next delta.
                let mut next_delta: HashMap<String, HashSet<Vec<Val>>> = HashMap::new();
                let mut changed = false;
                for (predicate, tuples) in derived {
                    let entry = self.idb.entry(predicate.clone()).or_default();
                    for tuple in tuples {
                        if entry.insert(tuple.clone()) {
                            next_delta
                                .entry(predicate.clone())
                                .or_default()
                                .insert(tuple);
                            changed = true;
                        }
                    }
                }

                if !changed {
                    break;
                }
                delta = next_delta;
                first_iteration = false;

                // Ground positive goals are monotone: once satisfied, further
                // strata cannot invalidate them.
                if self.all_queries_satisfied(program) {
                    queries_done = true;
                    break;
                }
            }
        }

        // Evaluate every `?-` query against the shared materialized IDB.
        let mut results = Vec::new();
        for (index, query) in program.queries.iter().enumerate() {
            let (_, rows) = self.evaluate_query(&query.goals);
            let name = query
                .name
                .clone()
                .unwrap_or_else(|| format!("query_{}", index + 1));
            results.push((name, rows));
        }
        let idb_snapshot = capture_idb.then(|| {
            self.idb
                .iter()
                .map(|(name, tuples)| {
                    (
                        name.clone(),
                        tuples.iter().cloned().collect::<Vec<Vec<Val>>>(),
                    )
                })
                .collect()
        });
        Ok((results, steps_used, idb_snapshot))
    }
}

/// Reject clauses whose inequality constraints reference variables that are
/// never bound by a positive body atom (mirrors Datalog negation safety).
fn validate_grounding(program: &DatalogProgram) -> Result<(), String> {
    for rule in &program.rules {
        validate_body_grounding(&rule.body, &format!("rule `{}`", rule.head.name))?;
    }
    for (index, query) in program.queries.iter().enumerate() {
        let label = query
            .name
            .clone()
            .unwrap_or_else(|| format!("query #{}", index + 1));
        validate_body_grounding(&query.goals, &format!("query `{label}`"))?;
    }
    Ok(())
}

fn validate_body_grounding(body: &[Atom], context: &str) -> Result<(), String> {
    let bound: HashSet<&str> = body
        .iter()
        .filter(|atom| !atom.negated && !atom.is_inequality())
        .flat_map(|atom| atom.terms.iter())
        .filter_map(|term| match term {
            Term::Var(var) => Some(var.as_str()),
            Term::Const(_) => None,
        })
        .collect();

    for atom in body.iter().filter(|atom| atom.is_inequality()) {
        for term in &atom.terms {
            if let Term::Var(var) = term {
                if !bound.contains(var.as_str()) {
                    return Err(format!(
                        "{context}: variable `{var}` appears only in an inequality constraint \
                         and is not bound by a positive body atom"
                    ));
                }
            }
        }
    }

    // Range restriction for negation: every named variable in a negated atom
    // must be bound before the negation runs. Anonymous `_` variables are
    // wildcards and are exempt.
    for atom in body.iter().filter(|atom| atom.negated) {
        for term in &atom.terms {
            if let Term::Var(var) = term {
                if !var.starts_with("__anon_") && !bound.contains(var.as_str()) {
                    return Err(format!(
                        "{context}: variable `{var}` appears in a negated atom `{}` but is not \
                         bound by a positive body atom (range restriction)",
                        atom.name
                    ));
                }
            }
        }
    }
    Ok(())
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

/// Machine-readable result for a single `?-` query.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatalogQueryResult {
    /// Optional `[label]` supplied by the user.
    pub name: Option<String>,
    /// Rendered query source, e.g. `?- has_proof_gap(X).`
    pub query: String,
    /// Projected variable names, in first-appearance order.
    pub variables: Vec<String>,
    /// Rows of projected values. String values are unquoted.
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub steps_used: usize,
    pub truncated: bool,
}

/// One materialized IDB relation included by `--show-derived`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatalogDerivedRelation {
    pub name: String,
    pub arity: usize,
    pub row_count: usize,
    /// Rows rendered like query rows (string values unquoted).
    pub rows: Vec<Vec<String>>,
}

/// Machine-readable report for a whole Datalog invocation (possibly several
/// `?-` queries evaluated against one shared EDB/IDB).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatalogReport {
    /// Schema identifier for consumers: always `"datalog.v1"`.
    pub schema_version: &'static str,
    /// Number of EDB tuples materialized from the graph.
    pub edb_facts: usize,
    pub queries: Vec<DatalogQueryResult>,
    /// Materialized IDB relations, present only when explicitly requested with
    /// `--show-derived` (or the equivalent library option). Goal-only output
    /// remains the default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived: Vec<DatalogDerivedRelation>,
}

/// Run a complete Datalog program and return a structured report.
pub fn run_datalog_report(
    graph: &crate::model::GrapheniumGraph,
    query: &str,
    step_budget: usize,
) -> Result<DatalogReport, String> {
    run_datalog_report_with_options(graph, query, step_budget, false)
}

/// Like [`run_datalog_report`], optionally including every materialized IDB
/// relation (`--show-derived`). Goal-only output remains the default.
pub fn run_datalog_report_with_options(
    graph: &crate::model::GrapheniumGraph,
    query: &str,
    step_budget: usize,
    include_derived: bool,
) -> Result<DatalogReport, String> {
    // Guard: query too long
    if query.len() > 10_000 {
        return Err("Query too long (max 10,000 characters)".to_string());
    }

    // Guard: max BASE (EDB) relation size — a sanity backstop only. The EDB is already
    // materialized (see load_from_graph below), so this does not prevent the EDB's own
    // memory; the real OOM risk is derived-relation blow-up during evaluation, which is
    // bounded by `step_budget` in `Interpreter::solve`. The previous 100k value rejected
    // every query on any real codebase (RobotStudio has ~225k edges), which defeated the
    // goal-directed lazy evaluation. Raised to a realistic backstop.
    let max_relation_size: usize = 10_000_000;

    // Step 1: Tokenize
    let tokens = tokenize(query)?;

    // Step 2: Parse and merge standard library
    let mut program = parse(&tokens)?;
    program.merge_stdlib(stdlib_rules().as_ref().clone());

    // Step 3: Load only the EDB relations reachable from the query goals.
    let edb_names = edb_relation_names();
    let needed = compute_needed_predicates_with(&program, &edb_names);
    let mut edb = Edb::default();
    edb.load_from_graph_selective(graph, &needed);

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
    let edb_facts = edb.relations.values().map(HashSet::len).sum();

    // Step 4: Evaluate every query against the shared materialized state.
    let mut interpreter = Interpreter::new(edb);
    let (results, steps_used, idb) = if include_derived {
        let (results, steps_used, idb) =
            interpreter.solve_with_steps_and_idb(&program, step_budget)?;
        (results, steps_used, Some(idb))
    } else {
        let (results, steps_used) = interpreter.solve_with_steps(&program, step_budget)?;
        (results, steps_used, None)
    };

    // Step 5: Build the structured report.
    let queries = program
        .queries
        .iter()
        .enumerate()
        .map(|(index, parsed)| {
            let rows = results
                .get(index)
                .map(|(_, rows)| rows.clone())
                .unwrap_or_default();
            let variables = query_variables(&parsed.goals);
            let rows: Vec<Vec<String>> = rows
                .iter()
                .map(|row| row.iter().map(json_value).collect())
                .collect();
            DatalogQueryResult {
                name: parsed.name.clone(),
                query: render_query(parsed),
                variables,
                row_count: rows.len(),
                rows,
                steps_used,
                truncated: false,
            }
        })
        .collect();

    let derived = idb
        .map(|idb| {
            let mut relations: Vec<DatalogDerivedRelation> = idb
                .into_iter()
                .map(|(name, mut tuples)| {
                    tuples.sort();
                    let arity = tuples.first().map(Vec::len).unwrap_or(0);
                    let row_count = tuples.len();
                    let rows = tuples
                        .iter()
                        .map(|row| row.iter().map(json_value).collect())
                        .collect();
                    DatalogDerivedRelation {
                        name,
                        arity,
                        row_count,
                        rows,
                    }
                })
                .collect();
            relations.sort_by(|a, b| a.name.cmp(&b.name));
            relations
        })
        .unwrap_or_default();

    Ok(DatalogReport {
        schema_version: "datalog.v1",
        edb_facts,
        queries,
        derived,
    })
}

/// Run a complete Datalog query against a graph with safety guardrails and
/// return human-readable Markdown.
pub fn run_datalog_query(
    graph: &crate::model::GrapheniumGraph,
    query: &str,
    step_budget: usize,
) -> Result<String, String> {
    run_datalog_query_with_options(graph, query, step_budget, false)
}

/// Like [`run_datalog_query`], optionally appending materialized IDB relations.
pub fn run_datalog_query_with_options(
    graph: &crate::model::GrapheniumGraph,
    query: &str,
    step_budget: usize,
    include_derived: bool,
) -> Result<String, String> {
    // Preserve the historical empty-graph sentinel for the text surface.
    if graph.node_count() == 0 {
        return Ok("(empty graph — no nodes to query)".to_string());
    }

    let report = run_datalog_report_with_options(graph, query, step_budget, include_derived)?;
    if report.queries.is_empty()
        || (report.queries.iter().all(|q| q.rows.is_empty()) && report.derived.is_empty())
    {
        return Ok("(no results)".to_string());
    }

    let mut output = String::new();
    for result in &report.queries {
        if result.rows.is_empty() {
            continue;
        }
        let title = result.name.clone().unwrap_or_else(|| result.query.clone());
        output.push_str(&format!("## Results for `{}`\n\n", title));
        if result.variables.is_empty() {
            output.push_str("| # | Values |\n|---|---|\n");
        } else {
            output.push_str(&format!(
                "| # | {} |\n|---|{}|\n",
                result.variables.join(" | "),
                "---|".repeat(result.variables.len())
            ));
        }
        for (i, row) in result.rows.iter().enumerate() {
            output.push_str(&format!("| {} | `{}` |\n", i + 1, row.join("`, `")));
        }
        output.push('\n');
    }

    for relation in &report.derived {
        output.push_str(&format!(
            "## Derived relation `{}({})`\n\n",
            relation.name,
            (0..relation.arity)
                .map(|index| format!("C{index}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        output.push_str(&format!("{} row(s)\n\n", relation.row_count));
        for (i, row) in relation.rows.iter().enumerate() {
            output.push_str(&format!("| {} | `{}` |\n", i + 1, row.join("`, `")));
        }
        output.push('\n');
    }

    Ok(output)
}

/// Serialize a Datalog report as pretty JSON (valid even when every result set
/// is empty).
pub fn run_datalog_json(
    graph: &crate::model::GrapheniumGraph,
    query: &str,
    step_budget: usize,
) -> Result<String, String> {
    run_datalog_json_with_options(graph, query, step_budget, false)
}

/// Like [`run_datalog_json`], optionally including every materialized IDB
/// relation under `derived`.
pub fn run_datalog_json_with_options(
    graph: &crate::model::GrapheniumGraph,
    query: &str,
    step_budget: usize,
    include_derived: bool,
) -> Result<String, String> {
    let report = run_datalog_report_with_options(graph, query, step_budget, include_derived)?;
    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
}

/// Projected variable names for a query, in first-appearance order.
fn query_variables(goals: &[Atom]) -> Vec<String> {
    let mut variables: Vec<String> = Vec::new();
    for atom in goals.iter().filter(|atom| !atom.is_inequality()) {
        for term in &atom.terms {
            if let Term::Var(var) = term {
                if !var.starts_with("__anon_") && !variables.contains(var) {
                    variables.push(var.clone());
                }
            }
        }
    }
    variables
}

fn json_value(value: &Val) -> String {
    match value {
        Val::Str(s) => s.to_string(),
        Val::Int(n) => n.to_string(),
    }
}

fn render_term(term: &Term) -> String {
    match term {
        Term::Var(var) => var.clone(),
        Term::Const(value) => value.to_string(),
    }
}

fn render_atom(atom: &Atom) -> String {
    if atom.is_inequality() {
        let left = atom.terms.first().map(render_term).unwrap_or_default();
        let right = atom.terms.get(1).map(render_term).unwrap_or_default();
        return format!("{left} != {right}");
    }
    let args = atom
        .terms
        .iter()
        .map(render_term)
        .collect::<Vec<_>>()
        .join(", ");
    let prefix = if atom.negated { "not " } else { "" };
    format!("{prefix}{}({args})", atom.name)
}

fn render_query(query: &DatalogQuery) -> String {
    let label = query
        .name
        .as_ref()
        .map(|name| format!("[{name}] "))
        .unwrap_or_default();
    let goals = query
        .goals
        .iter()
        .map(render_atom)
        .collect::<Vec<_>>()
        .join(", ");
    format!("?- {label}{goals}.")
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
                Val::Str(s) => s.to_string(),
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

    #[test]
    fn test_indexed_join_handles_large_fanout_graph_quickly() {
        let mut graph = crate::model::GrapheniumGraph::new();
        for i in 0..500 {
            graph.upsert_node(Node::new(
                &format!("node_{i}"),
                &format!("Node{i}"),
                FileType::Code,
                "src/large.rs",
            ));
        }
        // Create a 10-step chain from node_0 -> node_10 plus 480 unrelated edges
        for i in 0..10 {
            graph.add_edge(Edge::extracted(
                &format!("node_{i}"),
                &format!("node_{}", i + 1),
                "calls",
                "src/large.rs",
            ));
        }
        for i in 20..490 {
            graph.add_edge(Edge::extracted(
                &format!("node_{i}"),
                &format!("node_{}", i + 1),
                "calls",
                "src/large.rs",
            ));
        }

        let start = Instant::now();
        let result =
            run_datalog_query(&graph, r#"?- calls_transitive("node_0", "node_10")."#, 500).unwrap();
        assert!(
            start.elapsed().as_millis() < 500,
            "Indexed transitive query should complete in <500ms, took {:?}",
            start.elapsed()
        );
        assert!(!result.contains("no results"));
    }
}
