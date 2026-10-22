use crate::*;

/// Helper to construct integer attributes with specified bit width.
pub fn int_attr(ctx: &mut Context, width: u32, val: u64) -> IntegerAttr {
    let ty = IntegerType::get(ctx, width, Signedness::Signless);
    IntegerAttr::new(ty, APInt::from_u64(val, bw(width as usize)))
}

/// Parses numeric literals (sized base and unsized decimal) into `(value, width)`.
pub fn parse_number_literal(s: &str) -> (u64, u32) {
    if let Some((width_part, val_part)) = s.split_once('\'') {
        let width: u32 = width_part.parse().unwrap_or(32);
        let val_str =
            val_part.trim_start_matches(['b', 'B', 'd', 'D', 'h', 'H', 's', 'S', 'o', 'O']);
        let radix = if val_part.starts_with(['h', 'H']) || val_part.contains(['h', 'H']) {
            16
        } else if val_part.starts_with(['b', 'B']) || val_part.contains(['b', 'B']) {
            2
        } else if val_part.starts_with(['o', 'O']) || val_part.contains(['o', 'O']) {
            8
        } else {
            10
        };
        let value = u64::from_str_radix(&val_str.replace('_', ""), radix).unwrap_or(0);
        (value, width)
    } else {
        let value: u64 = s.replace('_', "").parse().unwrap_or(0);
        (value, 32)
    }
}
