use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidNumber(String),
    UnexpectedEOF(usize),
    UnterminatedString(usize),
    NoLeftBrace(usize),
    NoRightBrace(usize),
    InvalidBool(usize),
    NotSupportedChar(usize, char),
    ReferenceNotExist(String),
    FunctionNotExist(String),
    NotSupportedOp(String),
    BinaryOpNotRegistered(String),
    UnaryOpNotRegistered(String),
    ShouldBeNumber(),
    ShouldBeBool(),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNumber(s) => write!(f, "invalid number: {}", s),
            Self::UnexpectedEOF(start) => write!(f, "unexpected eof: {}", start),
            Self::UnterminatedString(start) => write!(f, "unterminated string: {}", start),
            Self::NoLeftBrace(start) => write!(f, "no left brace: {}", start),
            Self::NoRightBrace(start) => write!(f, "no right brace: {}", start),
            Self::InvalidBool(start) => write!(f, "invalid bool: {}", start),
            Self::NotSupportedChar(start, ch) => write!(f, "not supported char: {}, {}", start, ch),
            Self::ReferenceNotExist(name) => write!(f, "reference not exist: {}", name),
            Self::FunctionNotExist(name) => write!(f, "function not exist: {}", name),
            Self::NotSupportedOp(op) => write!(f, "not supported op: {}", op),
            Self::BinaryOpNotRegistered(op) => write!(f, "binary op not registered: {}", op),
            Self::UnaryOpNotRegistered(op) => write!(f, "unary op not registered: {}", op),
            Self::ShouldBeNumber() => write!(f, "should be number"),
            Self::ShouldBeBool() => write!(f, "should be bool"),
        }
    }
}