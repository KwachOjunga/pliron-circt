//! SystemVerilog parser targeting the `sv`, `hw`, and `seq` dialects.
//!
//! Parses synthesizable SystemVerilog module definitions into verified Pliron hardware IR
//! using the IEEE 1800-2017 compliant `sv-parser` library.
//!
//! The parser is largely as defined in [`sv-parser`](https://github.com/KwachOjunga/sv-parser.git).

#![allow(unused)]
mod core;
pub mod utils;

// external dependencies
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
use std::{
    string::{String, ToString},
    vec::Vec,
};
use sv_parser::{RefNode, SyntaxTree, parse_sv_str, unwrap_node};

// self-defined dependencies
use core::*;
use pliron_circt::{
    hw::{ModuleOp, OutputOp},
    seq::{ClockType, MemoryType, ResetType},
    sv::{
        AlwaysCombOp, AlwaysFfNoResetOp, AlwaysFfOp, AssignOp, BinaryExprOp, ConcatExprOp,
        ConstantExprOp, IndexExprOp, InstanceOp, LogicDeclOp, MemDeclOp, MuxExprOp, RegDeclOp,
        SliceExprOp, UnaryExprOp, WireDeclOp,
    },
};
use utils::*;

/// Convenience entry point to parse a SystemVerilog source string into a Pliron `ModuleOp`.
pub fn parse_sv_module(ctx: &mut Context, source: &str) -> Result<ModuleOp> {
    let defines = std::collections::HashMap::new();
    let (tree, _defines) = parse_sv_str(source, "", &defines, &[] as &[&str], false, false)
        .map_err(|e| verify_error!(Location::Unknown, "sv-parser syntax error: {:?}", e))?;

    let mut lowerer = SvModuleLowerer::new(ctx, source, &tree);
    lowerer.lower_module()
}
