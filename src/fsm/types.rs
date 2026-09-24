// SPDX-License-Identifier: Apache-2.0
// Copyright (c) The pliron contributors

//! Types for the `fsm` dialect.

use pliron::{
    context::Context,
    derive::pliron_type,
    r#type::{Type, TypeHandle},
};

/// An FSM instance handle type (`!fsm.instance`).
///
/// Represents an instance of a finite state machine.
#[pliron_type(name = "fsm.instance", format, verifier = "succ", generate_get = true)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceType;

/// An alternative spelling for FSM instance type (`!fsm.instancetype`).
///
/// Supported for compatibility with xDSL parity tests.
#[pliron_type(
    name = "fsm.instancetype",
    format,
    verifier = "succ",
    generate_get = true
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceTypeType;

/// Check if a type handle represents an FSM instance type (`!fsm.instance` or `!fsm.instancetype`).
pub fn is_instance_type(ctx: &Context, ty: TypeHandle) -> bool {
    let ty_ref = ty.deref(ctx);
    ty_ref.is::<InstanceType>() || ty_ref.is::<InstanceTypeType>()
}

/// Register all `fsm` types in [Context].
pub fn register(ctx: &mut Context) {
    InstanceType::register(ctx);
    InstanceTypeType::register(ctx);
}
