//! Parses a script and checks its types in one pass, producing the tree that
//! `eval` runs. Error messages are written to read like rustc's.

use super::ir::*;
use super::lexer::{lex, Pos, Token};
use super::CompileError;

type Result<T> = std::result::Result<T, CompileError>;

/// What compiling a script produces.
pub struct Compiled {
    /// The helper functions, by the index calls refer to.
    pub functions: Vec<Function>,
    pub main: Function,
    /// The sketch formula, if `main` shows a sketch.
    pub closure: Option<Closure>,
}

struct Sig {
    name: String,
    params: Vec<(String, Type)>,
    ret: Type,
    /// Where the `fn` keyword is, so the second pass can start there.
    at: usize,
}

struct Var {
    name: String,
    slot: usize,
    ty: Type,
    mutable: bool,
}

#[derive(PartialEq)]
enum Kind {
    Main,
    Helper,
    Closure,
}

struct Ctx {
    kind: Kind,
    scopes: Vec<Vec<Var>>,
    slots: usize,
    /// For a closure: `(slot in main, slot in the closure)` of what it copied.
    captures: Vec<(usize, usize)>,
}

impl Ctx {
    fn new(kind: Kind) -> Ctx {
        Ctx {
            kind,
            scopes: vec![Vec::new()],
            slots: 0,
            captures: Vec::new(),
        }
    }

    fn declare(&mut self, name: &str, ty: Type, mutable: bool) -> usize {
        let slot = self.slots;
        self.slots += 1;
        self.scopes.last_mut().expect("a scope").push(Var {
            name: name.to_string(),
            slot,
            ty,
            mutable,
        });
        slot
    }

    fn find(&self, name: &str) -> Option<&Var> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|var| var.name == name)
    }

    fn names(&self) -> Vec<&str> {
        self.scopes
            .iter()
            .flat_map(|scope| scope.iter().map(|var| var.name.as_str()))
            .collect()
    }
}

struct Compiler {
    tokens: Vec<(Token, Pos)>,
    at: usize,
    sigs: Vec<Sig>,
    ctxs: Vec<Ctx>,
    closure: Option<Closure>,
    scenes: usize,
    second_scene: Option<Pos>,
}

/// The edit distance between two names, for "did you mean".
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let current = row[j + 1];
            row[j + 1] = (previous + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            previous = current;
        }
    }
    row[b.len()]
}

/// The candidate closest to `name`, if any is close enough to be a typo.
fn closest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    candidates
        .into_iter()
        .map(|candidate| (distance(name, candidate), candidate))
        .filter(|(d, candidate)| *d <= 2 && *d < candidate.len().max(name.len()))
        .min_by_key(|(d, _)| *d)
        .map(|(_, candidate)| candidate)
}

fn plural(count: usize) -> String {
    if count == 1 {
        "1 argument".into()
    } else {
        format!("{count} arguments")
    }
}

fn supplied(count: usize) -> String {
    if count == 1 {
        "1 was supplied".into()
    } else {
        format!("{count} were supplied")
    }
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "if", "else", "for", "in", "as", "use", "true", "false", "return", "while",
    "loop", "match", "struct", "impl", "pub", "const", "static", "break", "continue",
];

pub fn compile_source(source: &str) -> Result<Compiled> {
    let tokens = lex(source).map_err(|e| CompileError {
        line: e.pos.line,
        column: e.pos.column,
        message: e.message,
    })?;
    let mut compiler = Compiler {
        tokens,
        at: 0,
        sigs: Vec::new(),
        ctxs: Vec::new(),
        closure: None,
        scenes: 0,
        second_scene: None,
    };
    compiler.collect_signatures()?;
    compiler.items()
}

impl Compiler {
    // ------------------------------------------------------------- tokens

    fn peek(&self) -> &Token {
        &self.tokens[self.at.min(self.tokens.len() - 1)].0
    }

    fn peek_at(&self, ahead: usize) -> &Token {
        &self.tokens[(self.at + ahead).min(self.tokens.len() - 1)].0
    }

    fn pos(&self) -> Pos {
        self.tokens[self.at.min(self.tokens.len() - 1)].1
    }

    fn bump(&mut self) -> (Token, Pos) {
        let item = self.tokens[self.at.min(self.tokens.len() - 1)].clone();
        if self.at < self.tokens.len() - 1 {
            self.at += 1;
        }
        item
    }

    fn is_punct(&self, text: &str) -> bool {
        matches!(self.peek(), Token::Punct(p) if *p == text)
    }

    fn is_ident(&self, text: &str) -> bool {
        matches!(self.peek(), Token::Ident(name) if name == text)
    }

    fn error<T>(&self, pos: Pos, message: impl Into<String>) -> Result<T> {
        Err(CompileError {
            line: pos.line,
            column: pos.column,
            message: message.into(),
        })
    }

    fn expect_punct(&mut self, text: &str) -> Result<Pos> {
        if self.is_punct(text) {
            Ok(self.bump().1)
        } else {
            let (found, pos) = (self.peek().clone(), self.pos());
            self.error(pos, format!("expected `{text}`, found {found}"))
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<(String, Pos)> {
        match self.peek().clone() {
            Token::Ident(name) => {
                let pos = self.bump().1;
                Ok((name, pos))
            }
            found => {
                let pos = self.pos();
                self.error(pos, format!("expected {what}, found {found}"))
            }
        }
    }

    fn expect_type(&self, want: Type, got: Type, pos: Pos) -> Result<()> {
        if want == got {
            Ok(())
        } else {
            self.error(
                pos,
                format!(
                    "mismatched types: expected {}, found {}",
                    want.describe(),
                    got.describe()
                ),
            )
        }
    }

    // ------------------------------------------------------------ pass one

    /// Finds every `fn` at the top level and reads its signature, so a
    /// function can be called before it is written.
    fn collect_signatures(&mut self) -> Result<()> {
        let mut depth = 0i32;
        let mut i = 0;
        while i < self.tokens.len() {
            match &self.tokens[i].0 {
                Token::Punct("{") => depth += 1,
                Token::Punct("}") => depth -= 1,
                Token::Ident(name) if name == "fn" && depth == 0 => {
                    self.at = i;
                    let sig = self.signature()?;
                    // skip the body
                    let mut nesting = 0i32;
                    loop {
                        match self.bump().0 {
                            Token::Punct("{") => nesting += 1,
                            Token::Punct("}") => {
                                nesting -= 1;
                                if nesting <= 0 {
                                    break;
                                }
                            }
                            Token::Eof => break,
                            _ => {}
                        }
                    }
                    self.sigs.push(sig);
                    i = self.at;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        self.at = 0;
        Ok(())
    }

    fn signature(&mut self) -> Result<Sig> {
        let at = self.at;
        self.bump(); // fn
        let (name, _) = self.expect_ident("a function name")?;
        self.expect_punct("(")?;
        let mut params = Vec::new();
        while !self.is_punct(")") {
            let (param, _) = self.expect_ident("a parameter name")?;
            self.expect_punct(":")?;
            let ty = self.parse_type()?;
            params.push((param, ty));
            if self.is_punct(",") {
                self.bump();
            } else {
                break;
            }
        }
        self.expect_punct(")")?;
        let ret = if self.is_punct("->") {
            self.bump();
            if name == "main" {
                // the return type of `main` is not checked; skip to the body
                while !self.is_punct("{") && *self.peek() != Token::Eof {
                    self.bump();
                }
                Type::ResultUnit
            } else {
                self.parse_type()?
            }
        } else {
            Type::Unit
        };
        Ok(Sig {
            name,
            params,
            ret,
            at,
        })
    }

    fn parse_type(&mut self) -> Result<Type> {
        let pos = self.pos();
        if self.is_punct("&") {
            self.bump();
            let (name, _) = self.expect_ident("a type")?;
            return if name == "Cell" {
                Ok(Type::Cell)
            } else {
                self.error(
                    pos,
                    format!("the type `&{name}` is not supported here; use `&Cell`"),
                )
            };
        }
        let (name, _) = self.expect_ident("a type")?;
        match name.as_str() {
            "f32" => Ok(Type::F32),
            "bool" => Ok(Type::Bool),
            "i32" | "usize" => Ok(Type::Int),
            _ => self.error(
                pos,
                format!(
                    "the type `{name}` is not supported here; use `f32`, `bool`, `i32` or `&Cell`"
                ),
            ),
        }
    }

    // ------------------------------------------------------------ pass two

    fn items(&mut self) -> Result<Compiled> {
        let mut functions: Vec<Option<Function>> = (0..self.sigs.len()).map(|_| None).collect();
        let mut main = None;

        loop {
            match self.peek().clone() {
                Token::Eof => break,
                Token::Ident(word) if word == "use" => {
                    while !self.is_punct(";") && *self.peek() != Token::Eof {
                        self.bump();
                    }
                    self.bump();
                }
                Token::Ident(word) if word == "fn" => {
                    let index = self
                        .sigs
                        .iter()
                        .position(|sig| sig.at == self.at)
                        .expect("every fn was seen in pass one");
                    let function = self.function(index)?;
                    if self.sigs[index].name == "main" {
                        main = Some(function);
                    } else {
                        functions[index] = Some(function);
                    }
                }
                found => {
                    let pos = self.pos();
                    return self.error(
                        pos,
                        format!("expected `fn` or `use`, found {}", found_text(&found)),
                    );
                }
            }
        }

        let Some(main) = main else {
            return self.error(
                Pos { line: 1, column: 1 },
                "the program needs a `fn main()`",
            );
        };
        if self.scenes == 0 {
            let at = self
                .sigs
                .iter()
                .find(|s| s.name == "main")
                .map(|s| self.tokens[s.at].1);
            return self.error(
                at.unwrap_or_default(),
                "`main` shows nothing: end it with `sketch(...)`, `sketch_with(...)` or `World::new()...run()`",
            );
        }
        if let Some(pos) = self.second_scene {
            return self.error(
                pos,
                "`main` can only show one scene: use one `sketch(...)` or one `World::new()...run()`",
            );
        }
        // main is not in the function table; keep indexes aligned by filling its slot
        let functions = functions
            .into_iter()
            .map(|f| {
                f.unwrap_or(Function {
                    params: 0,
                    slots: 0,
                    body: Block::default(),
                })
            })
            .collect();
        Ok(Compiled {
            functions,
            main,
            closure: self.closure.take(),
        })
    }

    fn function(&mut self, index: usize) -> Result<Function> {
        let (params, ret, is_main) = {
            let sig = &self.sigs[index];
            (sig.params.clone(), sig.ret, sig.name == "main")
        };
        // skip `fn name ( params ) -> type` to the body
        while !self.is_punct("{") {
            if *self.peek() == Token::Eof {
                let pos = self.pos();
                return self.error(pos, "expected `{`, found the end of the program");
            }
            self.bump();
        }

        let mut ctx = Ctx::new(if is_main { Kind::Main } else { Kind::Helper });
        for (name, ty) in &params {
            ctx.declare(name, *ty, false);
        }
        self.ctxs.push(ctx);
        let body_pos = self.pos();
        let result = self.block(is_main);
        let ctx = self.ctxs.pop().expect("the function context");
        let (body, tail_type) = result?;

        if !is_main {
            self.expect_type(ret, tail_type, body_pos)?;
        }
        Ok(Function {
            params: params.len(),
            slots: ctx.slots,
            body,
        })
    }

    // -------------------------------------------------------------- blocks

    fn ctx(&mut self) -> &mut Ctx {
        self.ctxs.last_mut().expect("a context")
    }

    /// `{ statements tail }`. In `main`'s own block, scenes are what it shows.
    fn block(&mut self, main_top: bool) -> Result<(Block, Type)> {
        self.expect_punct("{")?;
        self.ctx().scopes.push(Vec::new());
        let result = self.block_body(main_top);
        self.ctx().scopes.pop();
        let (stmts, tail, tail_type) = result?;
        self.expect_punct("}")?;
        Ok((Block { stmts, tail }, tail_type))
    }

    fn note_scene(&mut self, pos: Pos) {
        self.scenes += 1;
        if self.scenes == 2 {
            self.second_scene = Some(pos);
        }
    }

    fn block_body(&mut self, main_top: bool) -> Result<(Vec<Stmt>, Option<Expr>, Type)> {
        let mut stmts = Vec::new();
        loop {
            if self.is_punct("}") || *self.peek() == Token::Eof {
                return Ok((stmts, None, Type::Unit));
            }
            if self.is_punct(";") {
                self.bump();
                continue;
            }
            if self.is_ident("let") {
                stmts.push(self.let_statement()?);
                continue;
            }
            if self.is_ident("for") {
                stmts.push(self.for_statement()?);
                continue;
            }
            if let Token::Ident(name) = self.peek() {
                let assignment = matches!(
                    self.peek_at(1),
                    Token::Punct("=")
                        | Token::Punct("+=")
                        | Token::Punct("-=")
                        | Token::Punct("*=")
                        | Token::Punct("/=")
                );
                if assignment && !KEYWORDS.contains(&name.as_str()) {
                    stmts.push(self.assign_statement()?);
                    continue;
                }
            }

            let (expr, ty, pos) = self.expression(0)?;
            let ends_with_block = matches!(expr, Expr::If(..) | Expr::Block(_));
            if self.is_punct(";") {
                self.bump();
                stmts.push(self.statement_for(expr, ty, pos, main_top));
            } else if self.is_punct("}") {
                if main_top && ty == Type::Scene {
                    self.note_scene(pos);
                    stmts.push(Stmt::Show(expr));
                    return Ok((stmts, None, Type::Unit));
                }
                return Ok((stmts, Some(expr), ty));
            } else if ends_with_block {
                stmts.push(self.statement_for(expr, ty, pos, main_top));
            } else {
                let (found, at) = (self.peek().clone(), self.pos());
                return self.error(
                    at,
                    format!("expected `;` or `}}`, found {}", found_text(&found)),
                );
            }
        }
    }

    fn statement_for(&mut self, expr: Expr, ty: Type, pos: Pos, main_top: bool) -> Stmt {
        if main_top && ty == Type::Scene {
            self.note_scene(pos);
            Stmt::Show(expr)
        } else {
            Stmt::Expr(expr)
        }
    }

    fn let_statement(&mut self) -> Result<Stmt> {
        self.bump(); // let
        let mutable = if self.is_ident("mut") {
            self.bump();
            true
        } else {
            false
        };
        let (name, _) = self.expect_ident("a variable name")?;
        let annotation = if self.is_punct(":") {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_punct("=")?;
        let (expr, ty, pos) = self.expression(0)?;
        if let Some(annotation) = annotation {
            self.expect_type(annotation, ty, pos)?;
        }
        self.expect_punct(";")?;
        // declared after the value, so `let a = a + 1.0;` sees the old `a`
        let slot = self.ctx().declare(&name, ty, mutable);
        Ok(Stmt::Let(slot, expr))
    }

    fn assign_statement(&mut self) -> Result<Stmt> {
        let (name, pos) = self.expect_ident("a variable name")?;
        let (op_token, _) = self.bump();
        let op = match op_token {
            Token::Punct("+=") => Some(BinOp::Add),
            Token::Punct("-=") => Some(BinOp::Sub),
            Token::Punct("*=") => Some(BinOp::Mul),
            Token::Punct("/=") => Some(BinOp::Div),
            _ => None,
        };
        let Some((slot, ty, mutable)) = self.lookup(&name) else {
            return self.unknown_value(&name, pos);
        };
        if !mutable {
            return self.error(
                pos,
                format!("cannot assign twice to immutable variable `{name}`; declare it with `let mut {name}`"),
            );
        }
        let (expr, expr_type, expr_pos) = self.expression(0)?;
        self.expect_type(ty, expr_type, expr_pos)?;
        if op.is_some() && !matches!(ty, Type::F32 | Type::Int) {
            return self.error(
                pos,
                format!("cannot use that operator on {}", ty.describe()),
            );
        }
        self.expect_punct(";")?;
        Ok(Stmt::Assign(slot, op, expr))
    }

    fn for_statement(&mut self) -> Result<Stmt> {
        self.bump(); // for
        let (name, _) = self.expect_ident("a loop variable")?;
        if !self.is_ident("in") {
            let (found, pos) = (self.peek().clone(), self.pos());
            return self.error(pos, format!("expected `in`, found {}", found_text(&found)));
        }
        self.bump();
        let (from, from_type, from_pos) = self.expression(0)?;
        self.expect_type(Type::Int, from_type, from_pos)?;
        let inclusive = if self.is_punct("..=") {
            true
        } else if self.is_punct("..") {
            false
        } else {
            let (found, pos) = (self.peek().clone(), self.pos());
            return self.error(
                pos,
                format!("expected `..` or `..=`, found {}", found_text(&found)),
            );
        };
        self.bump();
        let (to, to_type, to_pos) = self.expression(0)?;
        self.expect_type(Type::Int, to_type, to_pos)?;

        self.ctx().scopes.push(Vec::new());
        let slot = self.ctx().declare(&name, Type::Int, false);
        let body = self.block(false);
        self.ctx().scopes.pop();
        let (body, body_type) = body?;
        if body_type != Type::Unit {
            let pos = self.pos();
            self.expect_type(Type::Unit, body_type, pos)?;
        }
        Ok(Stmt::For {
            slot,
            from,
            to,
            inclusive,
            body,
        })
    }

    // ----------------------------------------------------------- variables

    /// Finds a variable, copying it into the closure if it lives in `main`.
    fn lookup(&mut self, name: &str) -> Option<(usize, Type, bool)> {
        let last = self.ctxs.len() - 1;
        if let Some(var) = self.ctxs[last].find(name) {
            return Some((var.slot, var.ty, var.mutable));
        }
        if self.ctxs[last].kind == Kind::Closure && last > 0 {
            let outer = self.ctxs[last - 1].find(name).map(|v| (v.slot, v.ty))?;
            let inner = self.ctxs[last].slots;
            self.ctxs[last].slots += 1;
            self.ctxs[last].captures.push((outer.0, inner));
            // declare it in the closure's outermost scope so later uses share the copy
            self.ctxs[last].scopes[0].push(Var {
                name: name.to_string(),
                slot: inner,
                ty: outer.1,
                mutable: false,
            });
            return Some((inner, outer.1, false));
        }
        None
    }

    fn visible_names(&self) -> Vec<&str> {
        let mut names = self.ctxs.last().map(|c| c.names()).unwrap_or_default();
        if self.ctxs.last().is_some_and(|c| c.kind == Kind::Closure) && self.ctxs.len() > 1 {
            names.extend(self.ctxs[self.ctxs.len() - 2].names());
        }
        names
    }

    fn unknown_value<T>(&self, name: &str, pos: Pos) -> Result<T> {
        let mut message = format!("cannot find value `{name}` in this scope");
        let candidates = self
            .visible_names()
            .into_iter()
            .chain(CONSTANTS.iter().map(|(n, _)| *n));
        if let Some(suggestion) = closest(name, candidates) {
            message.push_str(&format!("; did you mean `{suggestion}`?"));
        }
        self.error(pos, message)
    }

    // --------------------------------------------------------- expressions

    /// Binding power of a binary operator and what it is.
    fn binary_op(&self) -> Option<(u8, BinOp)> {
        match self.peek() {
            Token::Punct("||") => Some((1, BinOp::Or)),
            Token::Punct("&&") => Some((2, BinOp::And)),
            Token::Punct("==") => Some((3, BinOp::Eq)),
            Token::Punct("!=") => Some((3, BinOp::Ne)),
            Token::Punct("<") => Some((3, BinOp::Lt)),
            Token::Punct("<=") => Some((3, BinOp::Le)),
            Token::Punct(">") => Some((3, BinOp::Gt)),
            Token::Punct(">=") => Some((3, BinOp::Ge)),
            Token::Punct("+") => Some((4, BinOp::Add)),
            Token::Punct("-") => Some((4, BinOp::Sub)),
            Token::Punct("*") => Some((5, BinOp::Mul)),
            Token::Punct("/") => Some((5, BinOp::Div)),
            Token::Punct("%") => Some((5, BinOp::Rem)),
            _ => None,
        }
    }

    fn expression(&mut self, min: u8) -> Result<(Expr, Type, Pos)> {
        let (mut left, mut left_type, start) = self.unary()?;
        loop {
            if self.is_ident("as") && 6 >= min {
                self.bump();
                let pos = self.pos();
                let (target, _) = self.expect_ident("a type")?;
                if target != "f32" {
                    return self.error(pos, "only `as f32` is supported here");
                }
                left = match left_type {
                    Type::Int => Expr::IntToFloat(Box::new(left)),
                    Type::F32 => left,
                    other => {
                        return self
                            .error(start, format!("cannot cast {} as `f32`", other.describe()));
                    }
                };
                left_type = Type::F32;
                continue;
            }
            let Some((power, op)) = self.binary_op() else {
                break;
            };
            if power < min {
                break;
            }
            self.bump();
            let (right, right_type, right_pos) = self.expression(power + 1)?;
            left_type = self.binary_type(op, left_type, right_type, start, right_pos)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok((left, left_type, start))
    }

    fn binary_type(
        &self,
        op: BinOp,
        left: Type,
        right: Type,
        left_pos: Pos,
        right_pos: Pos,
    ) -> Result<Type> {
        use BinOp::*;
        match op {
            And | Or => {
                self.expect_type(Type::Bool, left, left_pos)?;
                self.expect_type(Type::Bool, right, right_pos)?;
                Ok(Type::Bool)
            }
            Eq | Ne => {
                if !matches!(left, Type::F32 | Type::Int | Type::Bool) {
                    return self.error(left_pos, format!("cannot compare {}", left.describe()));
                }
                self.expect_type(left, right, right_pos)?;
                Ok(Type::Bool)
            }
            Lt | Le | Gt | Ge => {
                if !matches!(left, Type::F32 | Type::Int) {
                    return self.error(left_pos, format!("cannot order {}", left.describe()));
                }
                self.expect_type(left, right, right_pos)?;
                Ok(Type::Bool)
            }
            Add | Sub | Mul | Div | Rem => {
                if !matches!(left, Type::F32 | Type::Int) {
                    return self.error(
                        left_pos,
                        format!("cannot do arithmetic on {}", left.describe()),
                    );
                }
                self.expect_type(left, right, right_pos)?;
                Ok(left)
            }
        }
    }

    fn unary(&mut self) -> Result<(Expr, Type, Pos)> {
        let pos = self.pos();
        if self.is_punct("-") {
            self.bump();
            let (inner, ty, _) = self.unary()?;
            if !matches!(ty, Type::F32 | Type::Int) {
                return self.error(pos, format!("cannot negate {}", ty.describe()));
            }
            return Ok((Expr::Neg(Box::new(inner)), ty, pos));
        }
        if self.is_punct("!") {
            self.bump();
            let (inner, ty, inner_pos) = self.unary()?;
            self.expect_type(Type::Bool, ty, inner_pos)?;
            return Ok((Expr::Not(Box::new(inner)), Type::Bool, pos));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<(Expr, Type, Pos)> {
        let (mut expr, mut ty, start) = self.primary()?;
        loop {
            if self.is_punct("?") {
                self.bump();
                continue;
            }
            if !self.is_punct(".") {
                break;
            }
            self.bump();
            let (name, name_pos) = self.expect_ident("a field or method name")?;
            if self.is_punct("(") {
                let (next, next_type) = self.method(expr, ty, &name, name_pos)?;
                expr = next;
                ty = next_type;
            } else {
                let (next, next_type) = self.field(expr, ty, &name, name_pos)?;
                expr = next;
                ty = next_type;
            }
        }
        Ok((expr, ty, start))
    }

    fn field(&mut self, receiver: Expr, ty: Type, name: &str, pos: Pos) -> Result<(Expr, Type)> {
        if ty != Type::Cell {
            return self.error(pos, format!("no field `{name}` on type {}", ty.describe()));
        }
        match Field::ALL.iter().find(|(field, _, _)| *field == name) {
            Some((_, field, field_type)) => {
                Ok((Expr::Field(Box::new(receiver), *field), *field_type))
            }
            None => {
                let names: Vec<&str> = Field::ALL.iter().map(|(n, _, _)| *n).collect();
                let mut message = format!(
                    "no field `{name}` on type `Cell`; the fields are: {}",
                    names.join(", ")
                );
                if let Some(suggestion) = closest(name, names.iter().copied()) {
                    message.push_str(&format!(" (did you mean `{suggestion}`?)"));
                }
                self.error(pos, message)
            }
        }
    }

    /// Parses `(arg, arg, ...)`, returning each argument with its type and position.
    fn arguments(&mut self) -> Result<Vec<(Expr, Type, Pos)>> {
        self.expect_punct("(")?;
        let mut args = Vec::new();
        while !self.is_punct(")") {
            args.push(self.expression(0)?);
            if self.is_punct(",") {
                self.bump();
            } else {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(args)
    }

    fn check_count(&self, got: usize, wanted: usize, pos: Pos, what: &str) -> Result<()> {
        if got == wanted {
            Ok(())
        } else {
            self.error(
                pos,
                format!("this {what} takes {} but {}", plural(wanted), supplied(got)),
            )
        }
    }

    fn method(&mut self, receiver: Expr, ty: Type, name: &str, pos: Pos) -> Result<(Expr, Type)> {
        let args = self.arguments()?;
        let boxed = Box::new(receiver);
        match ty {
            Type::F32 => {
                if let Some((_, function)) = Math1::ALL.iter().find(|(n, _)| *n == name) {
                    self.check_count(args.len(), 0, pos, "method")?;
                    return Ok((Expr::Math1(*function, boxed), Type::F32));
                }
                if let Some((_, function)) = Math2::ALL.iter().find(|(n, _)| *n == name) {
                    self.check_count(args.len(), 1, pos, "method")?;
                    let (arg, arg_type, arg_pos) = args.into_iter().next().expect("one argument");
                    self.expect_type(Type::F32, arg_type, arg_pos)?;
                    return Ok((Expr::Math2(*function, boxed, Box::new(arg)), Type::F32));
                }
                match name {
                    "powi" => {
                        self.check_count(args.len(), 1, pos, "method")?;
                        let (arg, arg_type, arg_pos) =
                            args.into_iter().next().expect("one argument");
                        self.expect_type(Type::Int, arg_type, arg_pos)?;
                        Ok((Expr::Powi(boxed, Box::new(arg)), Type::F32))
                    }
                    "clamp" | "mul_add" => {
                        self.check_count(args.len(), 2, pos, "method")?;
                        let mut items = Vec::new();
                        for (arg, arg_type, arg_pos) in args {
                            self.expect_type(Type::F32, arg_type, arg_pos)?;
                            items.push(Box::new(arg));
                        }
                        let second = items.pop().expect("two arguments");
                        let first = items.pop().expect("two arguments");
                        let expr = if name == "clamp" {
                            Expr::Clamp(boxed, first, second)
                        } else {
                            Expr::MulAdd(boxed, first, second)
                        };
                        Ok((expr, Type::F32))
                    }
                    _ => {
                        let names = Math1::ALL
                            .iter()
                            .map(|(n, _)| *n)
                            .chain(Math2::ALL.iter().map(|(n, _)| *n))
                            .chain(["powi", "clamp", "mul_add"]);
                        self.no_method(name, "`f32`", closest(name, names), pos)
                    }
                }
            }
            Type::Cell if name == "dist_to_mouse" => {
                self.check_count(args.len(), 0, pos, "method")?;
                Ok((Expr::DistToMouse(boxed), Type::F32))
            }
            Type::World => match WorldSetting::ALL.iter().find(|(n, _, _)| *n == name) {
                Some((_, setting, wanted)) => {
                    self.check_count(args.len(), wanted.len(), pos, "method")?;
                    let mut exprs = Vec::new();
                    for ((arg, arg_type, arg_pos), want) in args.into_iter().zip(wanted.iter()) {
                        self.expect_type(*want, arg_type, arg_pos)?;
                        exprs.push(arg);
                    }
                    let result = if *setting == WorldSetting::Run {
                        Type::Scene
                    } else {
                        Type::World
                    };
                    Ok((Expr::WorldSetting(boxed, *setting, exprs), result))
                }
                None => {
                    let names = WorldSetting::ALL.iter().map(|(n, _, _)| *n);
                    self.no_method(name, "`World`", closest(name, names), pos)
                }
            },
            Type::Cell => {
                let suggestion = closest(name, ["dist_to_mouse"]);
                self.no_method(name, "`Cell`", suggestion, pos)
            }
            other => self.no_method(name, other.describe(), None, pos),
        }
    }

    fn no_method<T>(&self, name: &str, on: &str, suggestion: Option<&str>, pos: Pos) -> Result<T> {
        let mut message = format!("no method named `{name}` found for type {on}");
        if let Some(suggestion) = suggestion {
            message.push_str(&format!("; did you mean `{suggestion}`?"));
        }
        self.error(pos, message)
    }

    fn primary(&mut self) -> Result<(Expr, Type, Pos)> {
        let pos = self.pos();
        match self.peek().clone() {
            Token::Float(value) => {
                self.bump();
                Ok((Expr::Float(value), Type::F32, pos))
            }
            Token::Int(value) => {
                self.bump();
                Ok((Expr::Int(value), Type::Int, pos))
            }
            Token::Punct("(") => {
                self.bump();
                if self.is_punct(")") {
                    self.bump();
                    return Ok((Expr::Unit, Type::Unit, pos));
                }
                let (inner, ty, _) = self.expression(0)?;
                self.expect_punct(")")?;
                Ok((inner, ty, pos))
            }
            Token::Punct("{") => {
                let (block, ty) = self.block(false)?;
                Ok((Expr::Block(Box::new(block)), ty, pos))
            }
            Token::Punct("&") => {
                self.bump();
                self.expect_punct("[")?;
                let mut items = Vec::new();
                while !self.is_punct("]") {
                    let (item, ty, item_pos) = self.expression(0)?;
                    self.expect_type(Type::Colour, ty, item_pos)?;
                    items.push(item);
                    if self.is_punct(",") {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.expect_punct("]")?;
                Ok((Expr::Colours(items), Type::Colours, pos))
            }
            Token::Text => self.error(
                pos,
                "text and characters are not supported here; scripts are made of numbers",
            ),
            Token::Punct("|") | Token::Punct("||") => self.error(
                pos,
                "a closure like `|cell| ...` can only be given to `sketch(...)` or `sketch_with(...)`",
            ),
            Token::Ident(word) if word == "if" => self.if_expression(),
            Token::Ident(word) if word == "true" || word == "false" => {
                self.bump();
                Ok((Expr::Bool(word == "true"), Type::Bool, pos))
            }
            Token::Ident(word) if KEYWORDS.contains(&word.as_str()) => {
                self.error(pos, format!("`{word}` is not supported here"))
            }
            Token::Ident(_) => self.path(),
            found => self.error(pos, format!("expected an expression, found {}", found_text(&found))),
        }
    }

    fn if_expression(&mut self) -> Result<(Expr, Type, Pos)> {
        let pos = self.pos();
        self.bump(); // if
        let (condition, condition_type, condition_pos) = self.expression(0)?;
        self.expect_type(Type::Bool, condition_type, condition_pos)?;
        let (then, then_type) = self.block(false)?;

        if !self.is_ident("else") {
            if then_type != Type::Unit {
                return self.error(
                    pos,
                    format!(
                        "`if` may be missing an `else` clause: expected {}, found `()`",
                        then_type.describe()
                    ),
                );
            }
            return Ok((
                Expr::If(Box::new(condition), Box::new(then), None),
                Type::Unit,
                pos,
            ));
        }
        self.bump(); // else
        let else_pos = self.pos();
        let (otherwise, else_type) = if self.is_ident("if") {
            let (inner, ty, _) = self.if_expression()?;
            (
                Block {
                    stmts: Vec::new(),
                    tail: Some(inner),
                },
                ty,
            )
        } else {
            self.block(false)?
        };
        if then_type != else_type {
            return self.error(
                else_pos,
                format!(
                    "mismatched types: `if` and `else` have incompatible types: expected {}, found {}",
                    then_type.describe(),
                    else_type.describe()
                ),
            );
        }
        Ok((
            Expr::If(
                Box::new(condition),
                Box::new(then),
                Some(Box::new(otherwise)),
            ),
            then_type,
            pos,
        ))
    }

    // --------------------------------------------------------------- paths

    /// A name, or `a::b::c`, as a value or a call.
    fn path(&mut self) -> Result<(Expr, Type, Pos)> {
        let pos = self.pos();
        let mut segments = vec![self.expect_ident("a name")?.0];
        while self.is_punct("::") {
            self.bump();
            segments.push(self.expect_ident("a name")?.0);
        }
        let name = segments.last().expect("a segment").clone();

        if self.is_punct("(") {
            return self.call(&segments, pos);
        }

        if segments.len() == 1 {
            if let Some((slot, ty, _)) = self.lookup(&name) {
                return Ok((Expr::Local(slot), ty, pos));
            }
            if let Some((_, value)) = CONSTANTS.iter().find(|(n, _)| *n == name) {
                return Ok((Expr::Float(*value), Type::F32, pos));
            }
            return self.unknown_value(&name, pos);
        }

        let first = segments[0].as_str();
        if first == "Colour" && segments.len() == 2 {
            return match COLOURS.iter().find(|(n, _)| *n == name) {
                Some((_, colour)) => Ok((Expr::Colour(*colour), Type::Colour, pos)),
                None => {
                    let names: Vec<&str> = COLOURS.iter().map(|(n, _)| *n).collect();
                    let mut message = format!(
                        "no colour `{name}` on `Colour`; the colours are: {}",
                        names.join(", ")
                    );
                    if let Some(suggestion) = closest(&name, names.iter().copied()) {
                        message.push_str(&format!(" (did you mean `{suggestion}`?)"));
                    }
                    self.error(pos, message)
                }
            };
        }
        if matches!(first, "std" | "core" | "f32" | "consts") {
            if let Some((_, value)) = CONSTANTS.iter().find(|(n, _)| *n == name) {
                return Ok((Expr::Float(*value), Type::F32, pos));
            }
            return self.error(
                pos,
                format!("cannot find constant `{name}`; try `PI`, `TAU` or `E`"),
            );
        }
        self.unknown_type(first, pos)
    }

    fn unknown_type<T>(&self, name: &str, pos: Pos) -> Result<T> {
        self.error(
            pos,
            format!(
                "cannot find type `{name}` in this scope; this editor runs `sketch(...)` and \
                 `World` scripts, not programs that need a real terminal"
            ),
        )
    }

    fn call(&mut self, segments: &[String], pos: Pos) -> Result<(Expr, Type, Pos)> {
        let name = segments.last().expect("a segment").as_str();
        if segments.len() == 1 {
            return match name {
                "sketch" | "sketch_with" => self.sketch(name == "sketch_with", pos),
                "Ok" => {
                    let args = self.arguments()?;
                    self.check_count(args.len(), 1, pos, "function")?;
                    Ok((Expr::Unit, Type::ResultUnit, pos))
                }
                _ => self.user_call(name, pos),
            };
        }

        match segments[0].as_str() {
            "Palette" if segments.len() == 2 => {
                if let Some((_, kind)) = PaletteKind::ALL.iter().find(|(n, _)| *n == name) {
                    let args = self.arguments()?;
                    self.check_count(args.len(), 0, pos, "function")?;
                    return Ok((Expr::Palette(*kind), Type::Palette, pos));
                }
                if name == "new" {
                    let args = self.arguments()?;
                    self.check_count(args.len(), 1, pos, "function")?;
                    let (colours, ty, arg_pos) = args.into_iter().next().expect("one argument");
                    self.expect_type(Type::Colours, ty, arg_pos)?;
                    return Ok((Expr::PaletteNew(Box::new(colours)), Type::Palette, pos));
                }
                let names: Vec<&str> = PaletteKind::ALL
                    .iter()
                    .map(|(n, _)| *n)
                    .chain(["new"])
                    .collect();
                self.error(
                    pos,
                    format!(
                        "no function `{name}` on `Palette`; try one of: {}",
                        names.join(", ")
                    ),
                )
            }
            "World" if segments.len() == 2 && name == "new" => {
                let args = self.arguments()?;
                self.check_count(args.len(), 0, pos, "function")?;
                Ok((Expr::WorldNew, Type::World, pos))
            }
            "World" if segments.len() == 2 => self.error(
                pos,
                format!("no function `{name}` on `World`; start with `World::new()`"),
            ),
            "Colour" => self.error(pos, format!("`Colour::{name}` is a colour, not a function")),
            other => self.unknown_type(other, pos),
        }
    }

    fn user_call(&mut self, name: &str, pos: Pos) -> Result<(Expr, Type, Pos)> {
        let found = self
            .sigs
            .iter()
            .position(|sig| sig.name == name && sig.name != "main");
        let Some(index) = found else {
            let candidates: Vec<&str> = self.sigs.iter().map(|s| s.name.as_str()).collect();
            let mut message = format!("cannot find function `{name}` in this scope");
            if let Some(suggestion) = closest(
                name,
                candidates.iter().copied().chain(["sketch", "sketch_with"]),
            ) {
                message.push_str(&format!("; did you mean `{suggestion}`?"));
            }
            return self.error(pos, message);
        };
        let (params, ret) = (self.sigs[index].params.clone(), self.sigs[index].ret);
        let args = self.arguments()?;
        self.check_count(args.len(), params.len(), pos, "function")?;
        let mut exprs = Vec::new();
        for ((arg, arg_type, arg_pos), (_, want)) in args.into_iter().zip(params.iter()) {
            self.expect_type(*want, arg_type, arg_pos)?;
            exprs.push(arg);
        }
        Ok((Expr::Call(index, exprs), ret, pos))
    }

    // ------------------------------------------------------------- sketches

    fn sketch(&mut self, with_palette: bool, pos: Pos) -> Result<(Expr, Type, Pos)> {
        if self.ctxs.last().map(|c| &c.kind) != Some(&Kind::Main) {
            return self.error(pos, "`sketch` can only be used in `main`");
        }
        self.expect_punct("(")?;
        let palette = if with_palette {
            let (palette, ty, palette_pos) = self.expression(0)?;
            self.expect_type(Type::Palette, ty, palette_pos)?;
            self.expect_punct(",")?;
            Some(Box::new(palette))
        } else {
            None
        };
        let outer_slots = self.closure_argument()?;
        self.expect_punct(")")?;
        Ok((Expr::Sketch(palette, outer_slots), Type::Scene, pos))
    }

    /// `|cell| formula`: compiles the formula and returns which `main`
    /// variables it copied.
    fn closure_argument(&mut self) -> Result<Vec<usize>> {
        let pos = self.pos();
        if self.is_punct("||") {
            return self.error(
                pos,
                "the formula needs a parameter for the cell: `|cell| ...`",
            );
        }
        if !self.is_punct("|") {
            let found = self.peek().clone();
            return self.error(
                pos,
                format!(
                    "expected a formula like `|cell| ...`, found {}",
                    found_text(&found)
                ),
            );
        }
        if self.closure.is_some() {
            return self.error(pos, "a script can only have one formula");
        }
        self.bump();
        let (parameter, _) = self.expect_ident("the name of the cell parameter")?;
        if self.is_punct(":") {
            self.bump();
            let ty = self.parse_type()?;
            self.expect_type(Type::Cell, ty, pos)?;
        }
        if !self.is_punct("|") {
            let (found, at) = (self.peek().clone(), self.pos());
            return self.error(
                at,
                format!(
                    "expected `|` after the cell parameter, found {}",
                    found_text(&found)
                ),
            );
        }
        self.bump();

        let mut ctx = Ctx::new(Kind::Closure);
        ctx.declare(&parameter, Type::Cell, false);
        self.ctxs.push(ctx);
        let body = self.expression(0);
        let ctx = self.ctxs.pop().expect("the closure context");
        let (body, ty, body_pos) = body?;
        self.expect_type(Type::F32, ty, body_pos)?;

        let (outer, inner): (Vec<usize>, Vec<usize>) = ctx.captures.iter().copied().unzip();
        self.closure = Some(Closure {
            slots: ctx.slots,
            captures: inner,
            body: Block {
                stmts: Vec::new(),
                tail: Some(body),
            },
        });
        Ok(outer)
    }
}

fn found_text(token: &Token) -> String {
    token.to_string()
}
