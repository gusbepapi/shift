//! Named numeric conversions whose loss is intended.

/// Narrows to `f32` for formats that store 32-bit floats, such as
/// designspace coordinates, `F2Dot14` construction, and outline points.
#[expect(
    clippy::cast_possible_truncation,
    reason = "these formats store 32-bit floats; the lost precision is below a font unit"
)]
pub(crate) fn lossy_f32(value: f64) -> f32 {
    value as f32
}

/// Rounds to the nearest `i32`, saturating at the type's bounds; NaN becomes 0.
#[expect(
    clippy::cast_possible_truncation,
    reason = "float-to-integer `as` saturates, which is the wanted clamp"
)]
pub(crate) fn rounded_i32(value: f64) -> i32 {
    value.round() as i32
}

/// Truncates to a `u32`, saturating at the type's bounds; NaN and negatives become 0.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "float-to-integer `as` saturates, which is the wanted clamp"
)]
pub(crate) fn saturating_u32(value: f64) -> u32 {
    value as u32
}

/// Narrows an `i32` that the caller has already range-checked to a byte,
/// clamping instead of wrapping if that check is ever wrong.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the clamp keeps the value inside the byte range first"
)]
pub(crate) fn clamped_u8(value: i32) -> u8 {
    value.clamp(0, i32::from(u8::MAX)) as u8
}

/// Narrows an `i32` that the caller has already range-checked to `i16`,
/// clamping instead of wrapping if that check is ever wrong.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the clamp keeps the value inside the `i16` range first"
)]
pub(crate) fn clamped_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_conversions_clamp_instead_of_wrapping() {
        assert_eq!(rounded_i32(2.5), 3);
        assert_eq!(rounded_i32(f64::MAX), i32::MAX);
        assert_eq!(saturating_u32(-4.0), 0);
        assert_eq!(saturating_u32(f64::NAN), 0);
        assert_eq!(lossy_f32(0.1), 0.1_f32);
    }

    #[test]
    fn integer_narrowing_clamps_instead_of_wrapping() {
        assert_eq!(clamped_u8(300), 255);
        assert_eq!(clamped_u8(-1), 0);
        assert_eq!(clamped_u8(139), 139);
        assert_eq!(clamped_i16(40_000), i16::MAX);
        assert_eq!(clamped_i16(-40_000), i16::MIN);
        assert_eq!(clamped_i16(-7), -7);
    }
}
