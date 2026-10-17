//! What the compiler produces and the evaluator runs: a tree of typed,
//! name-resolved expressions. Variables are numbered slots, not names.

use terminal_graphics_interactive::{Cell, Colour, Palette, World};

/// The types a script can have. Whole numbers and decimals are different
/// types, as in Rust, so `cell.x * 2` is an error here just as it is there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    F32,
    /// A whole number: a literal, a loop counter, a count.
    Int,
    Bool,
    /// The `&Cell` a formula is given.
    Cell,
    Palette,
    Colour,
    /// A list of colours, `&[Colour::Black, Colour::Red]`.
    Colours,
    World,
    /// What `sketch(...)` and `World::new()...run()` make.
    Scene,
    /// The `|cell| ...` of a sketch.
    Closure,
    Unit,
    /// `Ok(())`.
    ResultUnit,
}

impl Type {
    /// How the type is named in error messages.
    pub fn describe(self) -> &'static str {
        match self {
            Type::F32 => "`f32`",
            Type::Int => "integer",
            Type::Bool => "`bool`",
            Type::Cell => "`&Cell`",
            Type::Palette => "`Palette`",
            Type::Colour => "`Colour`",
            Type::Colours => "`&[Colour]`",
            Type::World => "`World`",
            Type::Scene => "a scene",
            Type::Closure => "a closure",
            Type::Unit => "`()`",
            Type::ResultUnit => "`Result<(), _>`",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    X,
    Y,
    T,
    MouseX,
    MouseY,
    MouseDown,
    Width,
    Height,
}

impl Field {
    /// Every field with its name and type, in the order they are listed in errors.
    pub const ALL: [(&'static str, Field, Type); 8] = [
        ("x", Field::X, Type::F32),
        ("y", Field::Y, Type::F32),
        ("t", Field::T, Type::F32),
        ("mouse_x", Field::MouseX, Type::F32),
        ("mouse_y", Field::MouseY, Type::F32),
        ("mouse_down", Field::MouseDown, Type::Bool),
        ("width", Field::Width, Type::F32),
        ("height", Field::Height, Type::F32),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Math1 {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Sqrt,
    Cbrt,
    Abs,
    Floor,
    Ceil,
    Round,
    Trunc,
    Fract,
    Signum,
    Exp,
    Ln,
    Log10,
    Log2,
    Recip,
    ToRadians,
    ToDegrees,
}

impl Math1 {
    pub const ALL: [(&'static str, Math1); 25] = [
        ("sin", Math1::Sin),
        ("cos", Math1::Cos),
        ("tan", Math1::Tan),
        ("asin", Math1::Asin),
        ("acos", Math1::Acos),
        ("atan", Math1::Atan),
        ("sinh", Math1::Sinh),
        ("cosh", Math1::Cosh),
        ("tanh", Math1::Tanh),
        ("sqrt", Math1::Sqrt),
        ("cbrt", Math1::Cbrt),
        ("abs", Math1::Abs),
        ("floor", Math1::Floor),
        ("ceil", Math1::Ceil),
        ("round", Math1::Round),
        ("trunc", Math1::Trunc),
        ("fract", Math1::Fract),
        ("signum", Math1::Signum),
        ("exp", Math1::Exp),
        ("ln", Math1::Ln),
        ("log10", Math1::Log10),
        ("log2", Math1::Log2),
        ("recip", Math1::Recip),
        ("to_radians", Math1::ToRadians),
        ("to_degrees", Math1::ToDegrees),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Math2 {
    Powf,
    Atan2,
    Min,
    Max,
    Hypot,
    RemEuclid,
    Copysign,
}

impl Math2 {
    pub const ALL: [(&'static str, Math2); 7] = [
        ("powf", Math2::Powf),
        ("atan2", Math2::Atan2),
        ("min", Math2::Min),
        ("max", Math2::Max),
        ("hypot", Math2::Hypot),
        ("rem_euclid", Math2::RemEuclid),
        ("copysign", Math2::Copysign),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteKind {
    Ocean,
    Fire,
    Rainbow,
    Mono,
}

impl PaletteKind {
    pub const ALL: [(&'static str, PaletteKind); 4] = [
        ("ocean", PaletteKind::Ocean),
        ("fire", PaletteKind::Fire),
        ("rainbow", PaletteKind::Rainbow),
        ("mono", PaletteKind::Mono),
    ];

    pub fn palette(self) -> Palette {
        match self {
            PaletteKind::Ocean => Palette::ocean(),
            PaletteKind::Fire => Palette::fire(),
            PaletteKind::Rainbow => Palette::rainbow(),
            PaletteKind::Mono => Palette::mono(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldSetting {
    Balls,
    Size,
    Gravity,
    MousePulls,
    Trails,
    Palette,
    Run,
}

impl WorldSetting {
    /// Name, setting, and the types of its arguments.
    pub const ALL: [(&'static str, WorldSetting, &'static [Type]); 7] = [
        ("balls", WorldSetting::Balls, &[Type::Int]),
        ("size", WorldSetting::Size, &[Type::F32]),
        ("gravity", WorldSetting::Gravity, &[Type::F32]),
        ("mouse_pulls", WorldSetting::MousePulls, &[]),
        ("trails", WorldSetting::Trails, &[]),
        ("palette", WorldSetting::Palette, &[Type::Palette]),
        ("run", WorldSetting::Run, &[]),
    ];
}

/// The 16 colours by the name used in code.
pub const COLOURS: [(&str, Colour); 16] = [
    ("Black", Colour::Black),
    ("Red", Colour::Red),
    ("Green", Colour::Green),
    ("Yellow", Colour::Yellow),
    ("Blue", Colour::Blue),
    ("Magenta", Colour::Magenta),
    ("Cyan", Colour::Cyan),
    ("White", Colour::White),
    ("BrightBlack", Colour::BrightBlack),
    ("BrightRed", Colour::BrightRed),
    ("BrightGreen", Colour::BrightGreen),
    ("BrightYellow", Colour::BrightYellow),
    ("BrightBlue", Colour::BrightBlue),
    ("BrightMagenta", Colour::BrightMagenta),
    ("BrightCyan", Colour::BrightCyan),
    ("BrightWhite", Colour::BrightWhite),
];

/// Named constants (`std::f32::consts::PI` and friends).
pub const CONSTANTS: [(&str, f32); 10] = [
    ("PI", std::f32::consts::PI),
    ("TAU", std::f32::consts::TAU),
    ("E", std::f32::consts::E),
    ("FRAC_PI_2", std::f32::consts::FRAC_PI_2),
    ("FRAC_PI_3", std::f32::consts::FRAC_PI_3),
    ("FRAC_PI_4", std::f32::consts::FRAC_PI_4),
    ("FRAC_PI_6", std::f32::consts::FRAC_PI_6),
    ("SQRT_2", std::f32::consts::SQRT_2),
    ("LN_2", std::f32::consts::LN_2),
    ("LN_10", std::f32::consts::LN_10),
];

#[derive(Clone, Debug)]
pub enum Expr {
    Float(f32),
    Int(i64),
    Bool(bool),
    Unit,
    Local(usize),
    /// A field of the cell the first expression evaluates to.
    Field(Box<Expr>, Field),
    DistToMouse(Box<Expr>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Block>, Option<Box<Block>>),
    Block(Box<Block>),
    Math1(Math1, Box<Expr>),
    Math2(Math2, Box<Expr>, Box<Expr>),
    Powi(Box<Expr>, Box<Expr>),
    Clamp(Box<Expr>, Box<Expr>, Box<Expr>),
    MulAdd(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `as f32` on a whole number.
    IntToFloat(Box<Expr>),
    Call(usize, Vec<Expr>),
    Palette(PaletteKind),
    PaletteNew(Box<Expr>),
    Colour(Colour),
    Colours(Vec<Expr>),
    WorldNew,
    WorldSetting(Box<Expr>, WorldSetting, Vec<Expr>),
    /// `sketch(...)` / `sketch_with(...)`: the palette, if given, then the
    /// closure captures to copy out of the enclosing function.
    Sketch(Option<Box<Expr>>, Vec<usize>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let(usize, Expr),
    /// `name = value`, or `name op= value` when there is an operator.
    Assign(usize, Option<BinOp>, Expr),
    For {
        slot: usize,
        from: Expr,
        to: Expr,
        inclusive: bool,
        body: Block,
    },
    Expr(Expr),
    /// A scene in `main`: remember it as what the script shows.
    Show(Expr),
}

#[derive(Clone, Debug, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Expr>,
}

/// A helper function: its parameters are slots `0..params`.
#[derive(Clone, Debug)]
pub struct Function {
    pub params: usize,
    pub slots: usize,
    pub body: Block,
}

/// The formula of a sketch. Slot 0 is the cell.
#[derive(Clone, Debug)]
pub struct Closure {
    pub slots: usize,
    /// For each variable copied from `main`, in the order `Expr::Sketch` lists
    /// them: the closure slot it is copied into.
    pub captures: Vec<usize>,
    pub body: Block,
}

/// A value at run time.
#[derive(Clone, Debug)]
pub enum Value {
    Float(f32),
    Int(i64),
    Bool(bool),
    Cell(Cell),
    Palette(Palette),
    Colour(Colour),
    Colours(Vec<Colour>),
    World(World),
    Scene(crate::script::Scene),
    Unit,
}
