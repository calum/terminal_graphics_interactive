//! Runs the compiled tree. Everything is checked while compiling, so the only
//! things that can go wrong here are a script doing too much work, recursing
//! too deep, or dividing a whole number by zero.

use super::ir::*;
use super::Scene;
use terminal_graphics_interactive::{Cell, Palette, World};

/// The most node evaluations one call may use before it is stopped.
pub const STEP_BUDGET: i64 = 200_000;

/// How many helper functions may call each other, one inside another.
const MAX_DEPTH: usize = 24;

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeError(pub String);

type Result<T> = std::result::Result<T, RuntimeError>;

fn fail<T>(message: &str) -> Result<T> {
    Err(RuntimeError(message.to_string()))
}

pub struct Interp<'a> {
    functions: &'a [Function],
    stack: Vec<Value>,
    steps: i64,
    depth: usize,
    /// The scene `main` showed.
    pub scene: Option<Scene>,
    /// The values the sketch closure captured from `main`.
    pub captured: Vec<Value>,
}

impl<'a> Interp<'a> {
    pub fn new(functions: &'a [Function]) -> Interp<'a> {
        Interp {
            functions,
            stack: Vec::new(),
            steps: STEP_BUDGET,
            depth: 0,
            scene: None,
            captured: Vec::new(),
        }
    }

    /// Runs `main`, which shows the scene.
    pub fn run_main(&mut self, main: &Function) -> Result<()> {
        self.steps = STEP_BUDGET;
        self.depth = 0;
        self.stack.clear();
        self.stack.resize(main.slots, Value::Unit);
        self.block(&main.body, 0)?;
        Ok(())
    }

    /// Calls the sketch formula for one cell.
    pub fn call_closure(
        &mut self,
        closure: &Closure,
        captured: &[Value],
        cell: &Cell,
    ) -> Result<f32> {
        self.steps = STEP_BUDGET;
        self.depth = 0;
        self.stack.clear();
        self.stack.resize(closure.slots.max(1), Value::Unit);
        self.stack[0] = Value::Cell(cell.clone());
        // the variables copied from `main` live wherever the compiler put them
        for (value, slot) in captured.iter().zip(&closure.captures) {
            self.stack[*slot] = value.clone();
        }
        match self.block(&closure.body, 0)? {
            Value::Float(value) => Ok(value),
            _ => fail("the formula did not return a number"),
        }
    }

    fn block(&mut self, block: &Block, base: usize) -> Result<Value> {
        for stmt in &block.stmts {
            self.stmt(stmt, base)?;
        }
        match &block.tail {
            Some(tail) => self.eval(tail, base),
            None => Ok(Value::Unit),
        }
    }

    fn stmt(&mut self, stmt: &Stmt, base: usize) -> Result<()> {
        match stmt {
            Stmt::Let(slot, expr) => {
                let value = self.eval(expr, base)?;
                self.stack[base + slot] = value;
            }
            Stmt::Assign(slot, op, expr) => {
                let rhs = self.eval(expr, base)?;
                let value = match op {
                    Some(op) => binary(*op, self.stack[base + slot].clone(), rhs)?,
                    None => rhs,
                };
                self.stack[base + slot] = value;
            }
            Stmt::For {
                slot,
                from,
                to,
                inclusive,
                body,
            } => {
                let (from, to) = (self.int(from, base)?, self.int(to, base)?);
                let end = if *inclusive { to.saturating_add(1) } else { to };
                let mut i = from;
                while i < end {
                    self.spend()?;
                    self.stack[base + slot] = Value::Int(i);
                    self.block(body, base)?;
                    i += 1;
                }
            }
            Stmt::Expr(expr) => {
                self.eval(expr, base)?;
            }
            Stmt::Show(expr) => {
                if let Value::Scene(scene) = self.eval(expr, base)? {
                    self.scene = Some(scene);
                }
            }
        }
        Ok(())
    }

    /// Uses up one step, or stops a script that is doing too much.
    fn spend(&mut self) -> Result<()> {
        self.steps -= 1;
        if self.steps < 0 {
            return fail(
                "the script is doing too much work in one frame (is there a very long loop?)",
            );
        }
        Ok(())
    }

    fn float(&mut self, expr: &Expr, base: usize) -> Result<f32> {
        match self.eval(expr, base)? {
            Value::Float(value) => Ok(value),
            _ => fail("internal error: expected a number"),
        }
    }

    fn int(&mut self, expr: &Expr, base: usize) -> Result<i64> {
        match self.eval(expr, base)? {
            Value::Int(value) => Ok(value),
            _ => fail("internal error: expected a whole number"),
        }
    }

    fn cell(&mut self, expr: &Expr, base: usize) -> Result<Cell> {
        match self.eval(expr, base)? {
            Value::Cell(cell) => Ok(cell),
            _ => fail("internal error: expected a cell"),
        }
    }

    fn eval(&mut self, expr: &Expr, base: usize) -> Result<Value> {
        self.spend()?;
        Ok(match expr {
            Expr::Float(value) => Value::Float(*value),
            Expr::Int(value) => Value::Int(*value),
            Expr::Bool(value) => Value::Bool(*value),
            Expr::Unit => Value::Unit,
            Expr::Local(slot) => self.stack[base + slot].clone(),
            Expr::Field(cell, field) => {
                let cell = self.cell(cell, base)?;
                match field {
                    Field::X => Value::Float(cell.x),
                    Field::Y => Value::Float(cell.y),
                    Field::T => Value::Float(cell.t),
                    Field::MouseX => Value::Float(cell.mouse_x),
                    Field::MouseY => Value::Float(cell.mouse_y),
                    Field::MouseDown => Value::Bool(cell.mouse_down),
                    Field::Width => Value::Float(cell.width),
                    Field::Height => Value::Float(cell.height),
                }
            }
            Expr::DistToMouse(cell) => Value::Float(self.cell(cell, base)?.dist_to_mouse()),
            Expr::Neg(inner) => match self.eval(inner, base)? {
                Value::Float(value) => Value::Float(-value),
                Value::Int(value) => Value::Int(value.wrapping_neg()),
                _ => return fail("internal error: cannot negate that"),
            },
            Expr::Not(inner) => match self.eval(inner, base)? {
                Value::Bool(value) => Value::Bool(!value),
                _ => return fail("internal error: cannot apply `!` to that"),
            },
            Expr::Binary(BinOp::And, left, right) => match self.eval(left, base)? {
                Value::Bool(false) => Value::Bool(false),
                _ => self.eval(right, base)?,
            },
            Expr::Binary(BinOp::Or, left, right) => match self.eval(left, base)? {
                Value::Bool(true) => Value::Bool(true),
                _ => self.eval(right, base)?,
            },
            Expr::Binary(op, left, right) => {
                let left = self.eval(left, base)?;
                let right = self.eval(right, base)?;
                binary(*op, left, right)?
            }
            Expr::If(condition, then, otherwise) => match self.eval(condition, base)? {
                Value::Bool(true) => self.block(then, base)?,
                Value::Bool(false) => match otherwise {
                    Some(otherwise) => self.block(otherwise, base)?,
                    None => Value::Unit,
                },
                _ => return fail("internal error: the condition was not true or false"),
            },
            Expr::Block(block) => self.block(block, base)?,
            Expr::Math1(function, argument) => {
                let v = self.float(argument, base)?;
                Value::Float(math1(*function, v))
            }
            Expr::Math2(function, a, b) => {
                let (a, b) = (self.float(a, base)?, self.float(b, base)?);
                Value::Float(math2(*function, a, b))
            }
            Expr::Powi(value, power) => {
                let value = self.float(value, base)?;
                let power = self.int(power, base)?;
                Value::Float(value.powi(power.clamp(i32::MIN as i64, i32::MAX as i64) as i32))
            }
            Expr::Clamp(value, low, high) => {
                let (value, low, high) = (
                    self.float(value, base)?,
                    self.float(low, base)?,
                    self.float(high, base)?,
                );
                // f32::clamp panics when low > high or either is NaN; a script must not
                if low <= high {
                    Value::Float(value.clamp(low, high))
                } else {
                    Value::Float(f32::NAN)
                }
            }
            Expr::MulAdd(value, a, b) => {
                let (value, a, b) = (
                    self.float(value, base)?,
                    self.float(a, base)?,
                    self.float(b, base)?,
                );
                Value::Float(value.mul_add(a, b))
            }
            Expr::IntToFloat(inner) => Value::Float(self.int(inner, base)? as f32),
            Expr::Call(index, args) => {
                let function = &self.functions[*index];
                if self.depth >= MAX_DEPTH {
                    return fail("functions are calling each other too deep (is a function calling itself forever?)");
                }
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval(arg, base)?);
                }
                let new_base = self.stack.len();
                self.stack.extend(values);
                self.stack.resize(new_base + function.slots, Value::Unit);
                self.depth += 1;
                let result = self.block(&function.body, new_base);
                self.depth -= 1;
                self.stack.truncate(new_base);
                result?
            }
            Expr::Palette(kind) => Value::Palette(kind.palette()),
            Expr::PaletteNew(colours) => match self.eval(colours, base)? {
                Value::Colours(colours) => Value::Palette(Palette::new(&colours)),
                _ => return fail("internal error: expected a list of colours"),
            },
            Expr::Colour(colour) => Value::Colour(*colour),
            Expr::Colours(items) => {
                let mut colours = Vec::with_capacity(items.len());
                for item in items {
                    match self.eval(item, base)? {
                        Value::Colour(colour) => colours.push(colour),
                        _ => return fail("internal error: expected a colour"),
                    }
                }
                Value::Colours(colours)
            }
            Expr::WorldNew => Value::World(World::new()),
            Expr::WorldSetting(world, setting, args) => {
                let world = match self.eval(world, base)? {
                    Value::World(world) => world,
                    _ => return fail("internal error: expected a world"),
                };
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval(arg, base)?);
                }
                apply_setting(world, *setting, values)?
            }
            Expr::Sketch(palette, outer_slots) => {
                let palette = match palette {
                    Some(palette) => match self.eval(palette, base)? {
                        Value::Palette(palette) => palette,
                        _ => return fail("internal error: expected a palette"),
                    },
                    None => Palette::ocean(),
                };
                self.captured = outer_slots
                    .iter()
                    .map(|slot| self.stack[base + slot].clone())
                    .collect();
                Value::Scene(Scene::Sketch(palette))
            }
        })
    }
}

fn apply_setting(world: World, setting: WorldSetting, args: Vec<Value>) -> Result<Value> {
    Ok(match (setting, args.as_slice()) {
        (WorldSetting::Balls, [Value::Int(count)]) => {
            Value::World(world.balls(usize::try_from(*count).unwrap_or(0)))
        }
        (WorldSetting::Size, [Value::Float(radius)]) => Value::World(world.size(*radius)),
        (WorldSetting::Gravity, [Value::Float(gravity)]) => Value::World(world.gravity(*gravity)),
        (WorldSetting::MousePulls, []) => Value::World(world.mouse_pulls()),
        (WorldSetting::Trails, []) => Value::World(world.trails()),
        (WorldSetting::Palette, [Value::Palette(palette)]) => {
            Value::World(world.palette(palette.clone()))
        }
        (WorldSetting::Run, []) => Value::Scene(Scene::World(world)),
        _ => return fail("internal error: bad world setting"),
    })
}

fn binary(op: BinOp, left: Value, right: Value) -> Result<Value> {
    use BinOp::*;
    Ok(match (left, right) {
        (Value::Float(a), Value::Float(b)) => match op {
            Add => Value::Float(a + b),
            Sub => Value::Float(a - b),
            Mul => Value::Float(a * b),
            Div => Value::Float(a / b),
            Rem => Value::Float(a % b),
            Lt => Value::Bool(a < b),
            Le => Value::Bool(a <= b),
            Gt => Value::Bool(a > b),
            Ge => Value::Bool(a >= b),
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            And | Or => return fail("internal error: `&&` on numbers"),
        },
        (Value::Int(a), Value::Int(b)) => match op {
            Add => Value::Int(a.wrapping_add(b)),
            Sub => Value::Int(a.wrapping_sub(b)),
            Mul => Value::Int(a.wrapping_mul(b)),
            Div | Rem if b == 0 => return fail("attempt to divide by zero"),
            Div => Value::Int(a.wrapping_div(b)),
            Rem => Value::Int(a.wrapping_rem(b)),
            Lt => Value::Bool(a < b),
            Le => Value::Bool(a <= b),
            Gt => Value::Bool(a > b),
            Ge => Value::Bool(a >= b),
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            And | Or => return fail("internal error: `&&` on whole numbers"),
        },
        (Value::Bool(a), Value::Bool(b)) => match op {
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            And => Value::Bool(a && b),
            Or => Value::Bool(a || b),
            _ => return fail("internal error: arithmetic on true or false"),
        },
        _ => return fail("internal error: mismatched operands"),
    })
}

fn math1(function: Math1, v: f32) -> f32 {
    match function {
        Math1::Sin => v.sin(),
        Math1::Cos => v.cos(),
        Math1::Tan => v.tan(),
        Math1::Asin => v.asin(),
        Math1::Acos => v.acos(),
        Math1::Atan => v.atan(),
        Math1::Sinh => v.sinh(),
        Math1::Cosh => v.cosh(),
        Math1::Tanh => v.tanh(),
        Math1::Sqrt => v.sqrt(),
        Math1::Cbrt => v.cbrt(),
        Math1::Abs => v.abs(),
        Math1::Floor => v.floor(),
        Math1::Ceil => v.ceil(),
        Math1::Round => v.round(),
        Math1::Trunc => v.trunc(),
        Math1::Fract => v.fract(),
        Math1::Signum => v.signum(),
        Math1::Exp => v.exp(),
        Math1::Ln => v.ln(),
        Math1::Log10 => v.log10(),
        Math1::Log2 => v.log2(),
        Math1::Recip => v.recip(),
        Math1::ToRadians => v.to_radians(),
        Math1::ToDegrees => v.to_degrees(),
    }
}

fn math2(function: Math2, a: f32, b: f32) -> f32 {
    match function {
        Math2::Powf => a.powf(b),
        Math2::Atan2 => a.atan2(b),
        Math2::Min => a.min(b),
        Math2::Max => a.max(b),
        Math2::Hypot => a.hypot(b),
        Math2::RemEuclid => a.rem_euclid(b),
        Math2::Copysign => a.copysign(b),
    }
}
