use crate::context::Context;
use crate::define::*;
use crate::error::Error;
use crate::function::InnerFunctionManager;
use crate::operator::{BinOpType, BinaryOpFuncManager, UnaryOpFuncManager};
use crate::token::{DelimTokenType, Token};
use crate::tokenizer::Tokenizer;
use crate::value::Value;
use rust_decimal::prelude::*;
use std::fmt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Literal {
    Number(Decimal),
    Bool(bool),
    String(String),
}

#[cfg(not(tarpaulin_include))]
impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Literal::*;
        match self {
            Number(value) => write!(f, "Number: {}", value.clone()),
            Bool(value) => write!(f, "Bool: {}", value.clone()),
            String(value) => write!(f, "String: {}", value.clone()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExprAST {
    Literal(Literal),
    Unary(String, Box<ExprAST>),
    Binary(String, Box<ExprAST>, Box<ExprAST>),
    Ternary(Box<ExprAST>, Box<ExprAST>, Box<ExprAST>),
    Reference(String),
    Function(String, Vec<ExprAST>),
    List(Vec<ExprAST>),
    Map(Vec<(ExprAST, ExprAST)>),
    Chain(Vec<ExprAST>),
    None,
}

#[cfg(not(tarpaulin_include))]
impl fmt::Display for ExprAST {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(val) => write!(f, "Literal AST: {}", val.clone()),
            Self::Unary(op, rhs) => {
                write!(f, "Unary AST: Op: {}, Rhs: {}", op.clone(), rhs.clone())
            }
            Self::Binary(op, lhs, rhs) => write!(
                f,
                "Binary AST: Op: {}, Lhs: {}, Rhs: {}",
                op.clone(),
                lhs.clone(),
                rhs.clone()
            ),
            Self::Ternary(condition, lhs, rhs) => write!(
                f,
                "Ternary AST: Condition: {}, Lhs: {}, Rhs: {}",
                condition.clone(),
                lhs.clone(),
                rhs.clone()
            ),
            Self::Reference(name) => write!(f, "Reference AST: reference: {}", name.clone()),
            Self::Function(name, params) => {
                let mut s = "[".to_string();
                for param in params.into_iter() {
                    s.push_str(format!("{},", param.clone()).as_str());
                }
                s.push(']');
                write!(f, "Function AST: name: {}, params: {}", name.clone(), s)
            }
            Self::List(params) => {
                let mut s = "[".to_string();
                for param in params.into_iter() {
                    s.push_str(format!("{},", param.clone()).as_str());
                }
                s.push(']');
                write!(f, "List AST: params: {}", s)
            }
            Self::Map(m) => {
                let mut s = String::new();
                for (k, v) in m {
                    s.push_str(format!("({} {}), ", k.clone(), v.clone()).as_str());
                }
                write!(f, "Map AST: {}", s)
            }
            Self::Chain(exprs) => {
                let mut s = String::new();
                for expr in exprs {
                    s.push_str(format!("{};", expr.clone()).as_str());
                }
                write!(f, "Chain AST: {}", s)
            }
            Self::None => write!(f, "None"),
        }
    }
}

impl ExprAST {
    pub fn exec(&self, ctx: &mut Context) -> Result<Value> {
        use ExprAST::*;
        match self {
            Literal(literal) => self.exec_literal(literal.clone()),
            Reference(name) => self.exec_reference(name, ctx),
            Function(name, exprs) => self.exec_function(name, exprs.clone(), ctx),
            Unary(op, rhs) => self.exec_unary(op.clone(), rhs, ctx),
            Binary(op, lhs, rhs) => self.exec_binary(op.clone(), lhs, rhs, ctx),
            Ternary(condition, lhs, rhs) => self.exec_ternary(condition, lhs, rhs, ctx),
            List(params) => self.exec_list(params.clone(), ctx),
            Chain(exprs) => self.exec_chain(exprs.clone(), ctx),
            Map(m) => self.exec_map(m.clone(), ctx),
            None => Ok(Value::None),
        }
    }

    fn exec_literal(&self, literal: Literal) -> Result<Value> {
        match literal {
            Literal::Bool(value) => Ok(Value::from(value)),
            Literal::Number(value) => Ok(Value::from(value)),
            Literal::String(value) => Ok(Value::from(value)),
        }
    }

    fn exec_reference(&self, name: &str, ctx: &Context) -> Result<Value> {
        ctx.value(name)
    }

    fn exec_function(
        &self,
        name: &String,
        exprs: Vec<ExprAST>,
        ctx: &mut Context,
    ) -> Result<Value> {
        let mut params: Vec<Value> = Vec::new();
        for expr in exprs.into_iter() {
            params.push(expr.exec(ctx)?)
        }
        match ctx.get_func(name) {
            Some(func) => func(params),
            None => self.redirect_inner_function(name, params),
        }
    }

    fn redirect_inner_function(&self, name: &str, params: Vec<Value>) -> Result<Value> {
        InnerFunctionManager::new().get(name)?(params)
    }

    fn exec_unary(&self, op: String, rhs: &Box<ExprAST>, ctx: &mut Context) -> Result<Value> {
        UnaryOpFuncManager::new().get(&op)?(rhs.exec(ctx)?)
    }

    fn exec_binary(
        &self,
        op: String,
        lhs: &Box<ExprAST>,
        rhs: &Box<ExprAST>,
        ctx: &mut Context,
    ) -> Result<Value> {
        match BinaryOpFuncManager::new().get_op_type(&op)? {
            BinOpType::CALC => BinaryOpFuncManager::new().get(&op)?(lhs.exec(ctx)?, rhs.exec(ctx)?),
            BinOpType::SETTER => {
                let (a, b) = (lhs.exec(ctx)?, rhs.exec(ctx)?);
                ctx.set_variable(
                    lhs.get_reference_name()?.as_str(),
                    BinaryOpFuncManager::new().get(&op)?(a, b)?,
                );
                Ok(Value::None)
            }
        }
    }

    fn exec_ternary(
        &self,
        condition: &Box<ExprAST>,
        lhs: &Box<ExprAST>,
        rhs: &Box<ExprAST>,
        ctx: &mut Context,
    ) -> Result<Value> {
        match condition.exec(ctx)? {
            Value::Bool(val) => {
                if val {
                    return lhs.exec(ctx);
                }
                rhs.exec(ctx)
            }
            _ => Err(Error::ShouldBeBool()),
        }
    }

    fn exec_list(&self, params: Vec<ExprAST>, ctx: &mut Context) -> Result<Value> {
        let mut ans = Vec::new();
        for expr in params {
            ans.push(expr.exec(ctx)?);
        }
        Ok(Value::List(ans))
    }

    fn exec_chain(&self, params: Vec<ExprAST>, ctx: &mut Context) -> Result<Value> {
        let mut ans = Value::None;
        for expr in params {
            ans = expr.exec(ctx)?;
        }
        Ok(ans)
    }

    fn exec_map(&self, m: Vec<(ExprAST, ExprAST)>, ctx: &mut Context) -> Result<Value> {
        let mut ans = Vec::new();
        for (k, v) in m {
            ans.push((k.exec(ctx)?, v.exec(ctx)?));
        }
        Ok(Value::Map(ans))
    }

    pub fn expr(&self) -> String {
        match self {
            Self::Literal(val) => self.literal_expr(val.clone()),
            Self::Reference(name) => self.reference_expr(name.clone()),
            Self::Function(name, exprs) => self.function_expr(name.clone(), exprs.clone()),
            Self::Unary(op, rhs) => self.unary_expr(op, rhs),
            Self::Binary(op, lhs, rhs) => self.binary_expr(op, lhs, rhs),
            Self::Ternary(condition, lhs, rhs) => self.ternary_expr(condition, lhs, rhs),
            Self::List(params) => self.list_expr(params.clone()),
            Self::Map(m) => self.map_expr(m.clone()),
            Self::Chain(exprs) => self.chain_expr(exprs.clone()),
            Self::None => "".to_string(),
        }
    }

    fn literal_expr(&self, val: Literal) -> String {
        use Literal::*;
        match val {
            Number(value) => value.to_string(),
            Bool(value) => {
                if value {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            String(value) => "\"".to_string() + &value + "\"",
        }
    }

    fn reference_expr(&self, val: String) -> String {
        val
    }

    fn function_expr(&self, mut name: String, exprs: Vec<ExprAST>) -> String {
        name.push('(');
        for i in 0..exprs.len() {
            name.push_str(&exprs[i].expr());
            if i < exprs.len() - 1 {
                name.push(',');
            }
        }
        name.push(')');
        name
    }

    fn unary_expr(&self, op: &String, rhs: &Box<ExprAST>) -> String {
        op.clone() + " " + &rhs.expr()
    }

    fn binary_expr(&self, op: &String, lhs: &Box<ExprAST>, rhs: &Box<ExprAST>) -> String {
        let left = {
            let (is, precidence) = lhs.get_precidence();
            let mut tmp: String = lhs.expr();
            if is && precidence < BinaryOpFuncManager::new().get_precidence(op) {
                tmp = "(".to_string() + &lhs.expr() + &")".to_string();
            }
            tmp
        };
        let right = {
            let (is, precidence) = rhs.get_precidence();
            let mut tmp = rhs.expr();
            if is && precidence < BinaryOpFuncManager::new().get_precidence(op) {
                tmp = "(".to_string() + &rhs.expr() + &")".to_string();
            }
            tmp
        };
        left + " " + op + " " + &right
    }

    fn ternary_expr(
        &self,
        condition: &Box<ExprAST>,
        lhs: &Box<ExprAST>,
        rhs: &Box<ExprAST>,
    ) -> String {
        condition.expr() + " ? " + &lhs.expr() + " : " + &rhs.expr()
    }

    fn list_expr(&self, params: Vec<ExprAST>) -> String {
        let mut s = String::from("[");
        for i in 0..params.len() {
            s.push_str(params[i].expr().as_str());
            if i < params.len() - 1 {
                s.push_str(",");
            }
        }
        s.push_str("]");
        s
    }

    fn map_expr(&self, m: Vec<(ExprAST, ExprAST)>) -> String {
        let mut s = String::from("{");
        for i in 0..m.len() {
            let (key, value) = m[i].clone();
            s.push_str(key.expr().as_str());
            s.push_str(":");
            s.push_str(value.expr().as_str());
            if i < m.len() - 1 {
                s.push_str(",");
            }
        }
        s.push_str("}");
        s
    }

    fn chain_expr(&self, exprs: Vec<ExprAST>) -> String {
        let mut s = String::new();
        for i in 0..exprs.len() {
            s.push_str(exprs[i].expr().as_str());
            if i < exprs.len() - 1 {
                s.push_str(";");
            }
        }
        s
    }

    fn get_precidence(&self) -> (bool, i32) {
        match self {
            ExprAST::Binary(op, _, _) => (true, BinaryOpFuncManager::new().get_precidence(op)),
            _ => (false, 0),
        }
    }

    fn get_reference_name(&self) -> Result<String> {
        match &self {
            ExprAST::Reference(name) => Ok(name.clone()),
            _ => Err(Error::NotReferenceExpr),
        }
    }
}

pub struct Parser<'a> {
    tokenizer: Tokenizer<'a>,
}

impl<'a> Parser<'a> {
    fn cur_tok(&self) -> Token {
        self.tokenizer.cur_token.clone()
    }

    fn prev_tok(&self) -> Token {
        self.tokenizer.prev_token.clone()
    }

    pub fn new(input: &'a str) -> Result<Self> {
        let mut tokenizer = Tokenizer::new(input);
        tokenizer.next()?;
        Ok(Self {
            tokenizer: tokenizer,
        })
    }

    fn is_eof(&self) -> bool {
        self.cur_tok().is_eof()
    }

    pub fn next(&mut self) -> Result<Token> {
        self.tokenizer.next()
    }

    fn peek(&self) -> Result<Token> {
        self.tokenizer.peek()
    }

    fn expect(&mut self, expected: &str) -> Result<()> {
        self.tokenizer.expect(expected)
    }

    fn parse_token(&mut self) -> Result<ExprAST> {
        let token = self.cur_tok();
        match token {
            Token::Number(val, _) => {
                self.next()?;
                Ok(ExprAST::Literal(Literal::Number(val)))
            }
            Token::Bool(val, _) => {
                self.next()?;
                Ok(ExprAST::Literal(Literal::Bool(val)))
            }
            Token::String(val, _) => {
                self.next()?;
                Ok(ExprAST::Literal(Literal::String(val)))
            }
            Token::Reference(val, _) => {
                self.next()?;
                Ok(ExprAST::Reference(val))
            }
            Token::Function(name, _) => self.parse_function(name),
            Token::Operator(op, _) => self.parse_unary(op),
            Token::Delim(ty, _) => self.parse_delim(ty),
            Token::EOF => Err(Error::UnexpectedEOF(0)),
            _ => Err(Error::UnexpectedToken()),
        }
    }

    pub fn parse_chain_expression(&mut self) -> Result<ExprAST> {
        let mut ans = Vec::new();
        loop {
            if self.is_eof() {
                break;
            }
            ans.push(self.parse_expression()?);
            if self.cur_tok().is_semicolon() {
                self.next()?;
            }
        }
        if ans.len() == 1 {
            return Ok(ans[0].clone());
        }
        Ok(ExprAST::Chain(ans))
    }

    pub fn parse_expression(&mut self) -> Result<ExprAST> {
        let lhs = self.parse_primary()?;
        self.parse_op(0, lhs)
    }

    fn parse_primary(&mut self) -> Result<ExprAST> {
        Ok(self.parse_token()?)
    }

    fn parse_op(&mut self, exec_prec: i32, mut lhs: ExprAST) -> Result<ExprAST> {
        loop {
            if !self.cur_tok().is_op_token() {
                return Ok(lhs);
            }
            if self.cur_tok().is_question_mark() {
                self.next()?;
                let a = self.parse_expression()?;
                self.expect(":")?;
                let b = self.parse_expression()?;
                return Ok(ExprAST::Ternary(Box::new(lhs), Box::new(a), Box::new(b)));
            }
            let tok_prec = self.get_token_precidence();
            if tok_prec < exec_prec {
                return Ok(lhs);
            }
            let op = self.cur_tok().string();
            self.next()?;
            let mut rhs = self.parse_primary()?;
            if self.cur_tok().is_binop_token() && tok_prec < self.get_token_precidence() {
                rhs = self.parse_op(tok_prec + 1, rhs)?;
            }
            lhs = ExprAST::Binary(op, Box::new(lhs), Box::new(rhs))
        }
    }

    fn get_token_precidence(&self) -> i32 {
        match &self.cur_tok() {
            Token::Operator(op, _) => BinaryOpFuncManager::new().get_precidence(op),
            _ => -1,
        }
    }

    fn parse_delim(&mut self, ty: DelimTokenType) -> Result<ExprAST> {
        use DelimTokenType::*;
        match ty {
            OpenParen => self.parse_open_paren(),
            OpenBracket => self.parse_open_bracket(),
            OpenBrace => self.parse_open_brace(),
            _ => Err(Error::NoOpenDelim),
        }
    }

    fn parse_open_paren(&mut self) -> Result<ExprAST> {
        self.next()?;
        let expr = self.parse_expression()?;
        if !self.cur_tok().is_close_paren() {
            return Err(Error::NoCloseDelim);
        }
        self.next()?;
        Ok(expr)
    }

    fn parse_open_bracket(&mut self) -> Result<ExprAST> {
        self.next()?;
        let mut exprs = Vec::new();
        loop {
            if self.is_eof() || self.cur_tok().is_close_bracket() {
                break;
            }
            exprs.push(self.parse_expression()?);
            if !self.cur_tok().is_close_bracket() {
                self.expect(",")?;
            }
        }
        self.expect("]")?;
        Ok(ExprAST::List(exprs))
    }

    fn parse_open_brace(&mut self) -> Result<ExprAST> {
        self.next()?;
        let mut m = Vec::new();
        loop {
            if self.is_eof() || self.cur_tok().is_close_brace() {
                break;
            }
            let k = self.parse_expression()?;
            self.expect(":")?;
            let v = self.parse_expression()?;
            m.push((k, v));
            if !self.cur_tok().is_close_brace() {
                self.expect(",")?;
            }
        }
        self.expect("}")?;
        Ok(ExprAST::Map(m))
    }

    fn parse_unary(&mut self, op: String) -> Result<ExprAST> {
        self.next()?;
        Ok(ExprAST::Unary(op, Box::new(self.parse_primary()?)))
    }

    fn parse_function(&mut self, name: String) -> Result<ExprAST> {
        self.next()?;
        self.expect("(")?;
        let mut ans = Vec::new();
        if self.cur_tok().is_close_paren() {
            self.next()?;
            return Ok(ExprAST::Function(name, ans));
        }
        let has_right_paren;
        loop {
            ans.push(self.parse_expression()?);
            if self.cur_tok().is_close_paren() {
                has_right_paren = true;
                self.next()?;
                break;
            }
            self.expect(",")?;
        }
        if !has_right_paren {
            return Err(Error::NoCloseDelim);
        }
        Ok(ExprAST::Function(name, ans))
    }
}

#[cfg(test)]
mod tests {
    use crate::init::init;
    use crate::parser::{ExprAST, Literal, Parser};
    use crate::value::Value;
    use rstest::rstest;
    use rust_decimal::prelude::*;

    #[rstest]
    #[case("5", ExprAST::Literal(Literal::Number(Decimal::from_str("5").unwrap_or_default())))]
    #[case("true", ExprAST::Literal(Literal::Bool(true)))]
    #[case("\n false", ExprAST::Literal(Literal::Bool(false)))]
    #[case("\n haha", ExprAST::Reference("haha".to_string()))]
    #[case("'haha  '", ExprAST::Literal(Literal::String("haha  ".to_string())))]
    #[case("!a", ExprAST::Unary(
        "!".to_string(), Box::new(ExprAST::Reference("a".to_string()))
    ))]
    fn test_parse_expression_simple(#[case] input: &str, #[case] output: ExprAST) {
        init();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        assert!(expr_ast.is_ok());
        assert_eq!(expr_ast.unwrap(), output);
    }

    #[rstest]
    #[case("2+3*5", ExprAST::Binary(
        "+".to_string(), 
        Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(2).unwrap_or_default()))),
        Box::new(ExprAST::Binary(
            "*".to_string(),
            Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(3).unwrap_or_default()))),
            Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(5).unwrap_or_default()))),
        ))
    ))]
    #[case("(2+3)*5", ExprAST::Binary(
        "*".to_string(), 
        Box::new(ExprAST::Binary(
            "+".to_string(),
            Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(2).unwrap_or_default()))),
            Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(3).unwrap_or_default()))),
        )),
        Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(5).unwrap_or_default()))),
    ))]
    #[case("'hahhaff' beginWith 'hahha'", ExprAST::Binary(
        "beginWith".to_string(), 
        Box::new(ExprAST::Literal(Literal::String("hahhaff".to_string()))),
        Box::new(ExprAST::Literal(Literal::String("hahha".to_string()))),
    ))]
    fn test_parse_expression_binary(#[case] input: &str, #[case] output: ExprAST) {
        init();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        assert!(expr_ast.is_ok());
        assert_eq!(expr_ast.unwrap(), output);
    }

    #[rstest]
    #[case(" [] ", ExprAST::List(Vec::new()))]
    #[case("[1,!a,(2+3)*5,true, 'hahd', [1,!a,(2+3)*5,true, 'hahd']]", ExprAST::List(
        vec![
            ExprAST::Literal(Literal::Number(Decimal::from_str("1").unwrap_or_default())),
            ExprAST::Unary(
                "!".to_string(), Box::new(ExprAST::Reference("a".to_string()))
            ),
            ExprAST::Binary(
                "*".to_string(), 
                Box::new(ExprAST::Binary(
                    "+".to_string(),
                    Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(2).unwrap_or_default()))),
                    Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(3).unwrap_or_default()))),
                )),
                Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(5).unwrap_or_default()))),
            ),
            ExprAST::Literal(Literal::Bool(true)),
            ExprAST::Literal(Literal::String("hahd".to_string())),
            ExprAST::List(
                vec![
                    ExprAST::Literal(Literal::Number(Decimal::from_str("1").unwrap_or_default())),
                    ExprAST::Unary(
                        "!".to_string(), Box::new(ExprAST::Reference("a".to_string()))
                    ),
                    ExprAST::Binary(
                        "*".to_string(), 
                        Box::new(ExprAST::Binary(
                            "+".to_string(),
                            Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(2).unwrap_or_default()))),
                            Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(3).unwrap_or_default()))),
                        )),
                        Box::new(ExprAST::Literal(Literal::Number(Decimal::from_i32(5).unwrap_or_default()))),
                    ),
                    ExprAST::Literal(Literal::Bool(true)),
                    ExprAST::Literal(Literal::String("hahd".to_string())),
                ],
            ),
        ]
    ))]
    fn test_parse_expression_list(#[case] input: &str, #[case] output: ExprAST) {
        init();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        assert!(expr_ast.is_ok());
        assert_eq!(expr_ast.unwrap(), output);
    }

    #[rstest]
    #[case(" true ? 234:'haha'", ExprAST::Ternary(
        Box::new(ExprAST::Literal(Literal::Bool(true))),
        Box::new(ExprAST::Literal(Literal::Number(Decimal::from_str("234").unwrap_or_default()))), 
        Box::new(ExprAST::Literal(Literal::String("haha".to_string()))),
        )
    )]
    fn test_parse_expression_ternary(#[case] input: &str, #[case] output: ExprAST) {
        init();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        assert!(expr_ast.is_ok());
        assert_eq!(expr_ast.unwrap(), output);
    }

    #[rstest]
    #[case("  ")]
    #[case(" [ ")]
    #[case("[234,")]
    #[case(" { ")]
    #[case("{2:")]
    #[case("{2")]
    #[case("{2:}")]
    #[case(" (")]
    #[case("a(")]
    #[case("a(,)")]
    #[case("a(2,true,")]
    #[case("true ?")]
    #[case("true ? haha :")]
    #[case("2+ ")]
    fn test_parse_expression_error(#[case] input: &str) {
        init();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        assert!(expr_ast.is_err());
    }

    #[rstest]
    #[case(
        "
        a=3;
        a+=4;
        b=a+5;
        [a,b]
    ",
        ExprAST::Chain(
            vec![
                ExprAST::Binary(
                    "=".to_string(),
                    Box::new(ExprAST::Reference("a".to_string())),
                    Box::new(ExprAST::Literal(Literal::Number(Decimal::from_str("3").unwrap_or_default())))
                ),
                ExprAST::Binary(
                    "+=".to_string(),
                    Box::new(ExprAST::Reference("a".to_string())),
                    Box::new(ExprAST::Literal(Literal::Number(Decimal::from_str("4").unwrap_or_default())))
                ),
                ExprAST::Binary(
                    "=".to_string(),
                    Box::new(ExprAST::Reference("b".to_string())),
                    Box::new(ExprAST::Binary(
                        "+".to_string(),
                        Box::new(ExprAST::Reference("a".to_string())),
                        Box::new(ExprAST::Literal(Literal::Number(Decimal::from_str("5").unwrap_or_default())))
                    ))
                ),
                ExprAST::List(
                    vec![
                        ExprAST::Reference("a".to_string()),
                        ExprAST::Reference("b".to_string())
                    ]
                ),
            ]
        ),
    )]
    #[case("5", ExprAST::Literal(Literal::Number(Decimal::from_str("5").unwrap_or_default())))]
    #[case("true", ExprAST::Literal(Literal::Bool(true)))]
    #[case("\n false", ExprAST::Literal(Literal::Bool(false)))]
    #[case("\n haha", ExprAST::Reference("haha".to_string()))]
    #[case("'haha  '", ExprAST::Literal(Literal::String("haha  ".to_string())))]
    #[case("!a", ExprAST::Unary(
        "!".to_string(), Box::new(ExprAST::Reference("a".to_string()))
    ))]
    fn test_parse_chain_expression(#[case] input: &str, #[case] output: ExprAST) {
        init();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_chain_expression();
        assert!(expr_ast.is_ok());
        assert_eq!(expr_ast.unwrap(), output);
    }

    #[rstest]
    #[case("")]
    #[case(" ")]
    fn test_parse_chain_expression_error(#[case] input: &str) {
        init();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        assert!(expr_ast.is_err());
    }

    use crate::create_context;
    use crate::function::InnerFunctionManager;
    use std::sync::Arc;
    #[rstest]
    #[case("2", 2.into())]
    #[case("'haha'", "haha".into())]
    #[case("true", true.into())]
    #[case("  False", false.into())]
    #[case("!(2>3)", true.into())]
    #[case("2>3", false.into())]
    #[case(" 2<3 ", true.into())]
    #[case("2 >= 3", false.into())]
    #[case("2<=3", true.into())]
    #[case("2+3*5-2/2+6*(2+4 )-20", 32.into())]
    #[case("102%100",2.into())]
    #[case("2!=3", true.into())]
    #[case("2==3", false.into())]
    #[case("100>>3", (100>>3).into())]
    #[case("100<<3", (100<<3).into())]
    #[case("(2>3)&&true", false.into())]
    #[case("2>3||True", true.into())]
    #[case("d+=3;d", 6.into())]
    #[case("d-=2;d*5", 5.into())]
    #[case("d*=0.1;d+1.5", 1.8.into())]
    #[case("d/=2;d==1.5", true.into())]
    #[case("d%99;d", 3.into())]
    #[case("d<<=2;d", (3<<2).into())]
    #[case("d>>=2;d", (3>>2).into())]
    #[case("'hahhadf' beginWith \"hahha\"", true.into())]
    #[case("'hahhadf' endWith \"hahha\"", false.into())]
    #[case("true in [2, true, 'haha']", true.into())]
    #[case("-5*10", (-50).into())]
    #[case("AND[1>2,true]", false.into())]
    #[case("OR[1>2,true]", true.into())]
    #[case("[2>3,1+5]", Value::List(
        vec![false.into(),6.into()]
    ))]
    #[case("{'haha':2, 1+2:2>3}", Value::Map(
        vec![("haha".into(),2.into()),(3.into(),false.into())]
    ))]
    #[case("2<=3?'haha':false", "haha".into())]
    #[case("2>=3?'haha':false", false.into())]
    #[case("min(1,2,2+3*5,-10)", (-10).into())]
    #[case("max(1,2,2+3*5,-10)", 17.into())]
    #[case("mul(1,2,2+3*5,-10)", (-340).into())]
    #[case("sum(1,2,2+3*5,-10)", 10.into())]
    #[case("f(3)", 3.into())]
    #[case("d()", 4.into())]
    #[case("true in [2, true, 'haha']", true.into())]
    #[case("-5*10", (-50).into())]
    #[case("AND[1>2,true]", false.into())]
    #[case("OR[1>2,true]", true.into())]
    #[case("[2>3,1+5]", Value::List(
        vec![false.into(),6.into()]
    ))]
    #[case("{'haha':2, 1+2:2>3}", Value::Map(
        vec![("haha".into(),2.into()),(3.into(),false.into())]
    ))]
    #[case("2<=3?'haha':false", "haha".into())]
    #[case("2>=3?'haha':false", false.into())]
    fn test_exec(#[case] input: &str, #[case] output: Value) {
        init();
        let mut ctx = create_context!(
            "d" => 3,
            "f" => Arc::new(|_| Ok(Value::from(3)))
        );
        InnerFunctionManager::new().register("d", Arc::new(|_| Ok(4.into())));
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_chain_expression();
        assert!(expr_ast.is_ok());
        let ans = expr_ast.unwrap().exec(&mut ctx);
        assert!(ans.is_ok());
        assert_eq!(ans.unwrap(), output);
    }

    #[rstest]
    #[case("5", "5")]
    #[case(" true ", "true")]
    #[case(" True ", "true")]
    #[case(" false ", "false")]
    #[case(" False ", "false")]
    #[case("\n haha", "haha")]
    #[case("\t 'haha  '", "\"haha  \"")]
    #[case("!a", "! a")]
    #[case("not a", "not a")]
    #[case("2+3*5", "2 + 3 * 5")]
    #[case("(2+3)*5", "(2 + 3) * 5")]
    #[case("[]", "[]")]
    #[case(
        "[1,!a,(2+3)*5,true, 'hahd',[1,!a,(2+3)*5,true, 'hahd']]",
        "[1,! a,(2 + 3) * 5,true,\"hahd\",[1,! a,(2 + 3) * 5,true,\"hahd\"]]"
    )]
    #[case(" a()", "a()")]
    #[case(
        "test(1,!a,(2+3)*5,true, 'hahd',[1,!a,(2+3)*5,true, 'hahd'])",
        "test(1,! a,(2 + 3) * 5,true,\"hahd\",[1,! a,(2 + 3) * 5,true,\"hahd\"])"
    )]
    #[case("{}", "{}")]
    #[case("{2+3:5,'haha':d}", "{2 + 3:5,\"haha\":d}")]
    #[case("true?4: 2", "true ? 4 : 2")]
    #[case("2+3 >5?4: 2", "2 + 3 > 5 ? 4 : 2")]
    fn test_expression_expr(#[case] input: &str, #[case] output: &str) {
        init();
        let parser = Parser::new(input);
        assert!(parser.is_ok());
        let expr_ast = parser.unwrap().parse_expression();
        assert!(expr_ast.is_ok());
        assert_eq!(expr_ast.unwrap().expr(), output);
    }
}