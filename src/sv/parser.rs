// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! SystemVerilog parser targeting the `sv` and `hw` dialects.
//!
//! Parses synthesizable SystemVerilog module definitions, port signatures,
//! wire/logic declarations, continuous assignments, combinational blocks,
//! and sequential `always_ff` blocks into verified Pliron hardware IR.

use std::{
    string::{String, ToString},
    vec::Vec,
};
use awint::bw;
use pliron::{
    builtin::{
        attributes::IntegerAttr,
        types::{IntegerType, Signedness},
    },
    context::{Context, Ptr},
    identifier::Identifier,
    location::Location,
    op::Op,
    result::Result,
    r#type::{TypeHandle, Typed},
    utils::apint::APInt,
    value::Value,
    verify_err, verify_error,
};
use rustc_hash::FxHashMap;

use crate::{
    hw::ops::{ModuleOp, OutputOp},
    seq::types::{ClockType, ResetType},
    sv::ops::{
        AlwaysCombOp, AlwaysFfNoResetOp, AlwaysFfOp, AssignOp, BinaryExprOp, ConstantExprOp,
        LogicDeclOp, UnaryExprOp, WireDeclOp,
    },
};

/// Lexical tokens in synthesizable SystemVerilog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Module,
    EndModule,
    Input,
    Output,
    Inout,
    Logic,
    Wire,
    Reg,
    Assign,
    AlwaysFf,
    AlwaysComb,
    Posedge,
    Negedge,
    Begin,
    End,
    If,
    Else,
    Ident(String),
    Number { value: u64, width: u32 },
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Semicolon,
    Colon,
    Comma,
    At,
    Equal,        // =
    LessEqual,    // <=
    Plus,         // +
    Minus,        // -
    Star,         // *
    Slash,        // /
    Ampersand,    // &
    Pipe,         // |
    Caret,        // ^
    Tilde,        // ~
    Exclamation,  // !
    EqEqual,      // ==
    NotEqual,     // !=
    Less,         // <
    Greater,      // >
    GreaterEqual, // >=
}

/// Tokenizer for SystemVerilog source.
pub struct Lexer<'a> {
    src: &'a str,
    chars: core::iter::Peekable<core::str::CharIndices<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            chars: src.char_indices().peekable(),
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_whitespace() {
                self.chars.next();
            } else if ch == '/' {
                let mut lookahead = self.chars.clone();
                lookahead.next();
                if let Some((_, '/')) = lookahead.peek() {
                    // Line comment
                    self.chars.next();
                    self.chars.next();
                    for (_, c) in self.chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                } else if let Some((_, '*')) = lookahead.peek() {
                    // Block comment
                    self.chars.next();
                    self.chars.next();
                    let mut prev = ' ';
                    for (_, c) in self.chars.by_ref() {
                        if prev == '*' && c == '/' {
                            break;
                        }
                        prev = c;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace_and_comments();
        let (start, ch) = self.chars.next()?;

        match ch {
            '(' => Some(Token::LParen),
            ')' => Some(Token::RParen),
            '[' => Some(Token::LBracket),
            ']' => Some(Token::RBracket),
            '{' => Some(Token::LBrace),
            '}' => Some(Token::RBrace),
            ';' => Some(Token::Semicolon),
            ':' => Some(Token::Colon),
            ',' => Some(Token::Comma),
            '@' => Some(Token::At),
            '+' => Some(Token::Plus),
            '-' => Some(Token::Minus),
            '*' => Some(Token::Star),
            '/' => Some(Token::Slash),
            '&' => Some(Token::Ampersand),
            '|' => Some(Token::Pipe),
            '^' => Some(Token::Caret),
            '~' => Some(Token::Tilde),
            '!' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    Some(Token::NotEqual)
                } else {
                    Some(Token::Exclamation)
                }
            }
            '=' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    Some(Token::EqEqual)
                } else {
                    Some(Token::Equal)
                }
            }
            '<' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    Some(Token::LessEqual)
                } else {
                    Some(Token::Less)
                }
            }
            '>' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    Some(Token::GreaterEqual)
                } else {
                    Some(Token::Greater)
                }
            }
            '0'..='9' => {
                let mut end = start + ch.len_utf8();
                while let Some(&(idx, c)) = self.chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '\'' || c == '_' {
                        end = idx + c.len_utf8();
                        self.chars.next();
                    } else {
                        break;
                    }
                }
                let text = &self.src[start..end];
                Some(parse_number_literal(text))
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut end = start + ch.len_utf8();
                while let Some(&(idx, c)) = self.chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                        end = idx + c.len_utf8();
                        self.chars.next();
                    } else {
                        break;
                    }
                }
                let text = &self.src[start..end];
                match text {
                    "module" => Some(Token::Module),
                    "endmodule" => Some(Token::EndModule),
                    "input" => Some(Token::Input),
                    "output" => Some(Token::Output),
                    "inout" => Some(Token::Inout),
                    "logic" => Some(Token::Logic),
                    "wire" => Some(Token::Wire),
                    "reg" => Some(Token::Reg),
                    "assign" => Some(Token::Assign),
                    "always_ff" => Some(Token::AlwaysFf),
                    "always_comb" => Some(Token::AlwaysComb),
                    "posedge" => Some(Token::Posedge),
                    "negedge" => Some(Token::Negedge),
                    "begin" => Some(Token::Begin),
                    "end" => Some(Token::End),
                    "if" => Some(Token::If),
                    "else" => Some(Token::Else),
                    _ => Some(Token::Ident(text.to_string())),
                }
            }
            _ => None,
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(tok) = self.next_token() {
            tokens.push(tok);
        }
        tokens
    }
}

fn parse_number_literal(s: &str) -> Token {
    if let Some((width_part, val_part)) = s.split_once('\'') {
        let width: u32 = width_part.parse().unwrap_or(32);
        let val_str = val_part.trim_start_matches(['b', 'B', 'd', 'D', 'h', 'H']);
        let radix = if val_part.starts_with(['h', 'H']) {
            16
        } else if val_part.starts_with(['b', 'B']) {
            2
        } else {
            10
        };
        let value = u64::from_str_radix(&val_str.replace('_', ""), radix).unwrap_or(0);
        Token::Number { value, width }
    } else {
        let value: u64 = s.replace('_', "").parse().unwrap_or(0);
        Token::Number { value, width: 32 }
    }
}

/// Port definition parsed from a module header.
#[derive(Debug, Clone)]
pub struct ParsedPort {
    pub name: String,
    pub is_input: bool,
    pub is_output: bool,
    pub bit_width: u32,
    pub is_clock: bool,
    pub is_reset: bool,
}

/// SystemVerilog Parser producing Pliron `sv` and `hw` dialect operations.
pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    ctx: &'a mut Context,
    scope: FxHashMap<String, Value>,
}

impl<'a> Parser<'a> {
    pub fn new(ctx: &'a mut Context, tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            ctx,
            scope: FxHashMap::default(),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: Token) -> Result<()> {
        if let Some(tok) = self.advance() {
            if tok == expected {
                return Ok(());
            }
            return verify_err!(
                Location::Unknown,
                "expected token {:?}, found {:?}",
                expected,
                tok
            );
        }
        verify_err!(
            Location::Unknown,
            "unexpected EOF, expected {:?}",
            expected
        )
    }

    fn expect_ident(&mut self) -> Result<String> {
        if let Some(Token::Ident(id)) = self.advance() {
            Ok(id)
        } else {
            verify_err!(Location::Unknown, "expected identifier")
        }
    }

    fn parse_type_width(&mut self) -> Result<u32> {
        if let Some(Token::LBracket) = self.peek() {
            self.advance();
            let high = match self.advance() {
                Some(Token::Number { value, .. }) => value as u32,
                _ => return verify_err!(Location::Unknown, "expected bit range high bound"),
            };
            self.expect(Token::Colon)?;
            let low = match self.advance() {
                Some(Token::Number { value, .. }) => value as u32,
                _ => return verify_err!(Location::Unknown, "expected bit range low bound"),
            };
            self.expect(Token::RBracket)?;
            Ok(high - low + 1)
        } else {
            Ok(1)
        }
    }

    /// Parse an entire SystemVerilog module definition into an `hw::ops::ModuleOp`.
    pub fn parse_module(&mut self) -> Result<ModuleOp> {
        self.expect(Token::Module)?;
        let mod_name = self.expect_ident()?;
        self.expect(Token::LParen)?;

        let mut ports = Vec::new();

        while let Some(tok) = self.peek() {
            if *tok == Token::RParen {
                break;
            }

            let (is_input, is_output) = match self.advance() {
                Some(Token::Input) => (true, false),
                Some(Token::Output) => (false, true),
                Some(Token::Inout) => (true, true),
                other => {
                    return verify_err!(
                        Location::Unknown,
                        "expected input/output/inout port direction, got {:?}",
                        other
                    );
                }
            };

            if let Some(Token::Logic | Token::Wire | Token::Reg) = self.peek() {
                self.advance();
            }

            let width = self.parse_type_width()?;
            let port_name = self.expect_ident()?;

            let is_clock = (port_name == "clk" || port_name == "clock") && width == 1;
            let is_reset = (port_name == "rst" || port_name == "reset") && width == 1;

            ports.push(ParsedPort {
                name: port_name,
                is_input,
                is_output,
                bit_width: width,
                is_clock,
                is_reset,
            });

            if let Some(Token::Comma) = self.peek() {
                self.advance();
            } else {
                break;
            }
        }

        self.expect(Token::RParen)?;
        self.expect(Token::Semicolon)?;

        let mut input_types: Vec<TypeHandle> = Vec::new();
        for p in ports.iter().filter(|p| p.is_input) {
            let ty: TypeHandle = if p.is_clock {
                ClockType::get(self.ctx).into()
            } else if p.is_reset {
                ResetType::get(self.ctx).into()
            } else {
                IntegerType::get(self.ctx, p.bit_width, Signedness::Signless).into()
            };
            input_types.push(ty);
        }

        let module_ident: Identifier = mod_name
            .as_str()
            .try_into()
            .map_err(|_| verify_error!(Location::Unknown, "invalid module identifier"))?;
        let module = ModuleOp::new(self.ctx, module_ident, input_types);
        let body = module.get_body(self.ctx);

        let mut in_idx = 0;
        for p in &ports {
            if p.is_input {
                let arg_val = module.get_input(self.ctx, in_idx);
                if let Ok(id) = p.name.as_str().try_into() {
                    arg_val.set_name(self.ctx, Some(id));
                }
                self.scope.insert(p.name.clone(), arg_val);
                in_idx += 1;
            }
        }

        while let Some(tok) = self.peek() {
            if *tok == Token::EndModule {
                self.advance();
                break;
            }

            match tok {
                Token::Logic => {
                    self.advance();
                    let width = self.parse_type_width()?;
                    let name = self.expect_ident()?;
                    self.expect(Token::Semicolon)?;

                    let ty: TypeHandle =
                        IntegerType::get(self.ctx, width, Signedness::Signless).into();
                    let decl_op = LogicDeclOp::new(self.ctx, name.as_str(), ty);
                    let res = decl_op.result(self.ctx);
                    if let Ok(id) = name.as_str().try_into() {
                        res.set_name(self.ctx, Some(id));
                    }
                    decl_op.get_operation().insert_at_back(body, self.ctx);
                    self.scope.insert(name, res);
                }
                Token::Wire | Token::Reg => {
                    self.advance();
                    let width = self.parse_type_width()?;
                    let name = self.expect_ident()?;
                    self.expect(Token::Semicolon)?;

                    let ty: TypeHandle =
                        IntegerType::get(self.ctx, width, Signedness::Signless).into();
                    let decl_op = WireDeclOp::new(self.ctx, name.as_str(), ty);
                    let res = decl_op.result(self.ctx);
                    if let Ok(id) = name.as_str().try_into() {
                        res.set_name(self.ctx, Some(id));
                    }
                    decl_op.get_operation().insert_at_back(body, self.ctx);
                    self.scope.insert(name, res);
                }
                Token::Assign => {
                    self.advance();
                    let target_name = self.expect_ident()?;
                    self.expect(Token::Equal)?;
                    let expr_val = self.parse_expression(body)?;
                    self.expect(Token::Semicolon)?;

                    let assign_op = AssignOp::new(self.ctx, target_name.as_str(), expr_val);
                    let assigned = assign_op.result(self.ctx);
                    if let Ok(id) = target_name.as_str().try_into() {
                        assigned.set_name(self.ctx, Some(id));
                    }
                    assign_op.get_operation().insert_at_back(body, self.ctx);
                    self.scope.insert(target_name, assigned);
                }
                Token::AlwaysFf => {
                    self.advance();
                    self.expect(Token::At)?;
                    self.expect(Token::LParen)?;
                    self.expect(Token::Posedge)?;
                    let clk_name = self.expect_ident()?;
                    let clk_val = self.resolve_val(&clk_name)?;

                    let mut rst_name: Option<String> = None;
                    if let Some(Token::Ident(or_tok)) = self.peek() {
                        if or_tok == "or" {
                            self.advance();
                            if let Some(Token::Posedge | Token::Negedge) = self.peek() {
                                self.advance();
                            }
                            rst_name = Some(self.expect_ident()?);
                        }
                    }

                    self.expect(Token::RParen)?;
                    self.expect(Token::Begin)?;

                    if let Some(ref rst_id) = rst_name {
                        let rst_val = self.resolve_val(rst_id)?;
                        self.expect(Token::If)?;
                        self.expect(Token::LParen)?;
                        let _ = self.expect_ident()?;
                        self.expect(Token::RParen)?;
                        self.expect(Token::Begin)?;

                        let target_name = self.expect_ident()?;
                        self.expect(Token::LessEqual)?;
                        let reset_val_expr = self.parse_expression(body)?;
                        self.expect(Token::Semicolon)?;
                        self.expect(Token::End)?;

                        self.expect(Token::Else)?;
                        self.expect(Token::Begin)?;
                        let _ = self.expect_ident()?;
                        self.expect(Token::LessEqual)?;
                        let next_val_expr = self.parse_expression(body)?;
                        self.expect(Token::Semicolon)?;
                        self.expect(Token::End)?;
                        self.expect(Token::End)?;

                        let ff_op = AlwaysFfOp::new(
                            self.ctx,
                            target_name.as_str(),
                            clk_val,
                            next_val_expr,
                            rst_val,
                            reset_val_expr,
                            false,
                            "active_high",
                        );
                        ff_op.get_operation().insert_at_back(body, self.ctx);
                    } else {
                        let target_name = self.expect_ident()?;
                        self.expect(Token::LessEqual)?;
                        let next_val_expr = self.parse_expression(body)?;
                        self.expect(Token::Semicolon)?;
                        self.expect(Token::End)?;

                        let ff_op =
                            AlwaysFfNoResetOp::new(self.ctx, target_name.as_str(), clk_val, next_val_expr);
                        ff_op.get_operation().insert_at_back(body, self.ctx);
                    }
                }
                Token::AlwaysComb => {
                    self.advance();
                    self.expect(Token::Begin)?;
                    let target_name = self.expect_ident()?;
                    self.expect(Token::Equal)?;
                    let expr_val = self.parse_expression(body)?;
                    self.expect(Token::Semicolon)?;
                    self.expect(Token::End)?;

                    let comb_op = AlwaysCombOp::new(self.ctx, target_name.as_str(), expr_val);
                    comb_op.get_operation().insert_at_back(body, self.ctx);
                }
                _ => {
                    self.advance();
                }
            }
        }

        let mut output_vals: Vec<Value> = Vec::new();
        for p in ports.iter().filter(|p| p.is_output) {
            if let Some(val) = self.scope.get(&p.name) {
                output_vals.push(*val);
            } else {
                let z_val = int_attr(self.ctx, p.bit_width, 0);
                let w_val = int_attr(self.ctx, 32, p.bit_width as u64);
                let z_ty = IntegerType::get(self.ctx, p.bit_width, Signedness::Signless).into();
                let z_op = ConstantExprOp::new(self.ctx, z_val, w_val, z_ty);
                let res = z_op.result(self.ctx);
                z_op.get_operation().insert_at_back(body, self.ctx);
                output_vals.push(res);
            }
        }

        let out_op = OutputOp::new(self.ctx, output_vals);
        out_op.get_operation().insert_at_back(body, self.ctx);

        Ok(module)
    }

    fn resolve_val(&self, name: &str) -> Result<Value> {
        self.scope
            .get(name)
            .copied()
            .ok_or_else(|| verify_error!(Location::Unknown, "unresolved identifier '{}'", name))
    }

    /// Parse a SystemVerilog expression producing a Pliron `Value`.
    fn parse_expression(&mut self, body: Ptr<pliron::basic_block::BasicBlock>) -> Result<Value> {
        let lhs = self.parse_primary_or_unary(body)?;

        if let Some(tok) = self.peek() {
            let op_str = match tok {
                Token::Plus => Some("+"),
                Token::Minus => Some("-"),
                Token::Star => Some("*"),
                Token::Slash => Some("/"),
                Token::Ampersand => Some("&"),
                Token::Pipe => Some("|"),
                Token::Caret => Some("^"),
                Token::EqEqual => Some("=="),
                Token::NotEqual => Some("!="),
                Token::Less => Some("<"),
                Token::Greater => Some(">"),
                Token::LessEqual => Some("<="),
                Token::GreaterEqual => Some(">="),
                _ => None,
            };

            if let Some(op_symbol) = op_str {
                self.advance();
                let rhs = self.parse_expression(body)?;
                let res_ty = lhs.get_type(self.ctx);

                let bin_op = BinaryExprOp::new(self.ctx, op_symbol, lhs, rhs, res_ty);
                let res = bin_op.result(self.ctx);
                bin_op.get_operation().insert_at_back(body, self.ctx);
                return Ok(res);
            }
        }

        Ok(lhs)
    }

    fn parse_primary_or_unary(&mut self, body: Ptr<pliron::basic_block::BasicBlock>) -> Result<Value> {
        match self.peek() {
            Some(Token::Tilde) => {
                self.advance();
                let operand = self.parse_primary_or_unary(body)?;
                let res_ty = operand.get_type(self.ctx);
                let un_op = UnaryExprOp::new(self.ctx, "~", operand, res_ty);
                let res = un_op.result(self.ctx);
                un_op.get_operation().insert_at_back(body, self.ctx);
                Ok(res)
            }
            Some(Token::Exclamation) => {
                self.advance();
                let operand = self.parse_primary_or_unary(body)?;
                let res_ty = operand.get_type(self.ctx);
                let un_op = UnaryExprOp::new(self.ctx, "!", operand, res_ty);
                let res = un_op.result(self.ctx);
                un_op.get_operation().insert_at_back(body, self.ctx);
                Ok(res)
            }
            Some(Token::Ident(id)) => {
                let id = id.clone();
                self.advance();
                self.resolve_val(&id)
            }
            Some(Token::Number { value, width }) => {
                let (val, w) = (*value, *width);
                self.advance();
                let v_attr = int_attr(self.ctx, w, val);
                let w_attr = int_attr(self.ctx, 32, w as u64);
                let ty = IntegerType::get(self.ctx, w, Signedness::Signless).into();
                let c_op = ConstantExprOp::new(self.ctx, v_attr, w_attr, ty);
                let res = c_op.result(self.ctx);
                c_op.get_operation().insert_at_back(body, self.ctx);
                Ok(res)
            }
            Some(Token::LParen) => {
                self.advance();
                let inner = self.parse_expression(body)?;
                self.expect(Token::RParen)?;
                Ok(inner)
            }
            other => verify_err!(
                Location::Unknown,
                "expected primary expression, found {:?}",
                other
            ),
        }
    }
}

fn int_attr(ctx: &mut Context, width: u32, val: u64) -> IntegerAttr {
    let ty = IntegerType::get(ctx, width, Signedness::Signless);
    IntegerAttr::new(ty, APInt::from_u64(val, bw(width as usize)))
}

/// Convenience entry point to parse a SystemVerilog source string into a Pliron `ModuleOp`.
pub fn parse_sv_module(ctx: &mut Context, source: &str) -> Result<ModuleOp> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(ctx, tokens);
    parser.parse_module()
}
