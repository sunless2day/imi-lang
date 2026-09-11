//! in this module, spanned wraps around multiple types
//! which is proof of why using generics earlier was a good idea

use crate::token::Spanned;

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    /// an array literal is simply a list of expressions
    Array(Vec<ExprNode>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Str,
    Bool,
    /// classic recursive type error
    /// an enum variant can't contain itself directly because the size would be infinite
    /// but storing it as a heap pointer solves the issue because pointers have a known, finite size
    Array(Box<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOp {
    Assign,
    AddEq,
    SubEq,
    MulEq,
    DivEq,
    PowEq,
    ModEq,
}

pub type ExprNode = Spanned<Expr>;

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Variable(String),
    Unary(UnaryOp, Box<ExprNode>),
    Binary(Box<ExprNode>, BinaryOp, Box<ExprNode>),
    /// functions can only be called by name (there are no function values in imi),
    /// so the callee is stored as a plain String, not an expression
    Call(String, Vec<ExprNode>),
    /// (target, index), for example: `arr[i]`
    Index(Box<ExprNode>, Box<ExprNode>),
    /// (object, method name, arguments), for example: `arr.push(x)`
    MethodCall(Box<ExprNode>, String, Vec<ExprNode>),
}

/// a parameter is just an identifier and a type, like `a: int`
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

pub type StmtNode = Spanned<Stmt>;

/// note that some parts are wrapped around an Option<T> precisely because they are optional
#[derive(Debug, Clone)]
pub enum Stmt {
    /// declares an immutable variable (assigning to it after it's initialized is an error)
    Let(String, Option<Type>, Option<ExprNode>),
    /// declares a mutable variable
    Var(String, Option<Type>, Option<ExprNode>),
    Assign(String, AssignOp, ExprNode),
    /// (target, index, op, value), for example: `arr[i] += 2`
    IndexAssign(ExprNode, ExprNode, AssignOp, ExprNode),
    Block(Vec<StmtNode>),
    /// (condition, then block, optional else block)
    If(ExprNode, Vec<StmtNode>, Option<Vec<StmtNode>>),
    While(ExprNode, Vec<StmtNode>),
    Continue,
    Break,
    Return(Option<ExprNode>),
    ExprStmt(ExprNode),
    /// (name, parameters, optional return type, body)
    /// for example, a function may return nothing, so the type is optional
    FnDecl(String, Vec<Param>, Option<Type>, Vec<StmtNode>),
}

/// a little helper later used in the evaluator module
/// I moved this block of code here instead of copy-pasting the same thing in different places
/// it simply maps an assignment operator to its respective operation... yeah, that's it
impl AssignOp {
    pub fn to_binary_op(&self) -> Option<BinaryOp> {
        match self {
            AssignOp::AddEq => Some(BinaryOp::Add),
            AssignOp::SubEq => Some(BinaryOp::Sub),
            AssignOp::MulEq => Some(BinaryOp::Mul),
            AssignOp::DivEq => Some(BinaryOp::Div),
            AssignOp::PowEq => Some(BinaryOp::Pow),
            AssignOp::ModEq => Some(BinaryOp::Mod),
            AssignOp::Assign => None,
        }
    }
}

/// this is necessary for better error messages
impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::Str => write!(f, "str"),
            Type::Bool => write!(f, "bool"),
            // rust infers inner's type as &Box<Type> so it naturally allows nesting
            Type::Array(inner) => write!(f, "array[{}]", inner),
        }
    }
}
