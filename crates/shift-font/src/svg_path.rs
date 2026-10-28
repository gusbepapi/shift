//! Contours from SVG path data, as authoring payloads and pasted SVG carry
//! outlines.

use svgtypes::{SimplePathSegment, SimplifyingPathParser};

use crate::{Contour, PointType};

/// Why SVG path data does not describe contours.
#[derive(Debug, thiserror::Error)]
pub enum SvgPathError {
    #[error("invalid SVG path data: {0}")]
    Parse(#[from] svgtypes::Error),

    #[error("SVG path data must begin with M or m")]
    MissingMoveTo,

    #[error("SVG path data has a coordinate that is not finite")]
    NotFinite,
}

/// The contours SVG path data draws, one per subpath, in font units as
/// given (no axis is flipped).
///
/// Relative commands become absolute, `H` and `V` lines, `S` and `T`
/// reflect the previous control, and arcs become cubics. A line or curve
/// adds an on-curve point, a cubic two off-curve points before it, and a
/// quadratic one before a `qCurve` point. `Z` closes the subpath, dropping
/// a final on-curve point that repeats the start, since closing already
/// joins the two; one without `Z` stays open. Every point is a corner, with a fresh id.
///
/// # Errors
///
/// Returns [`SvgPathError::Parse`] for malformed path data,
/// [`SvgPathError::MissingMoveTo`] when drawing starts before a move, and
/// [`SvgPathError::NotFinite`] for a coordinate that is NaN or infinite.
pub fn contours_from_svg_path(path: &str) -> Result<Vec<Contour>, SvgPathError> {
    let mut contours = Vec::<Contour>::new();
    for segment in SimplifyingPathParser::from(path) {
        let segment = segment?;
        if let SimplePathSegment::MoveTo { x, y } = segment {
            let mut contour = Contour::new();
            contour.add_point(x, y, PointType::OnCurve, false);
            contours.push(contour);
            continue;
        }
        let contour = contours.last_mut().ok_or(SvgPathError::MissingMoveTo)?;
        match segment {
            SimplePathSegment::LineTo { x, y } => {
                contour.add_point(x, y, PointType::OnCurve, false);
            }
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                contour.add_point(x1, y1, PointType::OffCurve, false);
                contour.add_point(x2, y2, PointType::OffCurve, false);
                contour.add_point(x, y, PointType::OnCurve, false);
            }
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                contour.add_point(x1, y1, PointType::OffCurve, false);
                contour.add_point(x, y, PointType::QCurve, false);
            }
            SimplePathSegment::ClosePath => {
                drop_repeated_start(contour);
                contour.close();
            }
            SimplePathSegment::MoveTo { .. } => unreachable!("a move starts a new contour"),
        }
    }

    let finite = contours
        .iter()
        .flat_map(Contour::points)
        .all(|point| point.x().is_finite() && point.y().is_finite());
    if !finite {
        return Err(SvgPathError::NotFinite);
    }
    Ok(contours)
}

/// Removes a final point that sits exactly where the contour starts, so a
/// path that draws back to its start does not close on a doubled point.
fn drop_repeated_start(contour: &mut Contour) {
    let points = contour.points();
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return;
    };
    let repeats_start = points.len() > 1
        && last.point_type() == PointType::OnCurve
        && last.x() == first.x()
        && last.y() == first.y();
    if repeats_start {
        contour.points_mut().pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drawn point: its coordinates and type.
    type Drawn = (f64, f64, PointType);

    /// Each contour as whether it is closed and its points.
    fn shape(contours: &[Contour]) -> Vec<(bool, Vec<Drawn>)> {
        contours
            .iter()
            .map(|contour| {
                let points = contour
                    .points()
                    .iter()
                    .map(|point| (point.x(), point.y(), point.point_type()))
                    .collect();
                (contour.is_closed(), points)
            })
            .collect()
    }

    use PointType::{OffCurve as Off, OnCurve as On, QCurve as Q};

    #[test]
    fn lines_and_closing_make_one_closed_contour() {
        let contours = contours_from_svg_path("M0,0 L100,0 L100,100 Z").unwrap();

        assert_eq!(
            shape(&contours),
            [(
                true,
                vec![(0.0, 0.0, On), (100.0, 0.0, On), (100.0, 100.0, On)]
            )]
        );
    }

    #[test]
    fn a_closing_point_that_repeats_the_start_is_dropped() {
        let contours = contours_from_svg_path("M0,0 L100,0 L100,100 L0,0 Z").unwrap();

        assert_eq!(
            shape(&contours),
            [(
                true,
                vec![(0.0, 0.0, On), (100.0, 0.0, On), (100.0, 100.0, On)]
            )]
        );
    }

    #[test]
    fn off_curve_points_that_curve_back_to_the_start_are_kept() {
        let contours = contours_from_svg_path("M0,0 L100,0 C100,50 50,100 0,0 Z").unwrap();

        assert_eq!(
            shape(&contours),
            [(
                true,
                vec![
                    (0.0, 0.0, On),
                    (100.0, 0.0, On),
                    (100.0, 50.0, Off),
                    (50.0, 100.0, Off)
                ]
            )]
        );
    }

    #[test]
    fn relative_horizontal_and_vertical_commands_become_absolute_points() {
        let contours = contours_from_svg_path("M10,10 l20,0 v20 H0").unwrap();

        assert_eq!(
            shape(&contours),
            [(
                false,
                vec![
                    (10.0, 10.0, On),
                    (30.0, 10.0, On),
                    (30.0, 30.0, On),
                    (0.0, 30.0, On)
                ]
            )]
        );
    }

    #[test]
    fn cubics_and_quadratics_add_their_controls() {
        let contours = contours_from_svg_path("M0,0 C25,50 75,50 100,0 Q150,50 200,0").unwrap();

        assert_eq!(
            shape(&contours),
            [(
                false,
                vec![
                    (0.0, 0.0, On),
                    (25.0, 50.0, Off),
                    (75.0, 50.0, Off),
                    (100.0, 0.0, On),
                    (150.0, 50.0, Off),
                    (200.0, 0.0, Q),
                ]
            )]
        );
    }

    #[test]
    fn smooth_commands_reflect_the_previous_control() {
        let contours = contours_from_svg_path("M0,0 C10,20 30,20 40,0 S70,-20 80,0").unwrap();

        assert_eq!(shape(&contours)[0].1[4], (50.0, -20.0, Off));
    }

    #[test]
    fn each_subpath_is_its_own_contour() {
        let contours = contours_from_svg_path("M0,0 L100,0 Z M50,50 L150,50 Z").unwrap();

        assert_eq!(contours.len(), 2);
        assert!(contours.iter().all(Contour::is_closed));
    }

    #[test]
    fn arcs_become_cubics_ending_where_the_arc_does() {
        let contours = contours_from_svg_path("M0,0 A50,50 0 0 1 100,0").unwrap();
        let points = contours[0].points();

        assert!(points.iter().any(|point| point.point_type() == Off));
        let last = points.last().unwrap();
        assert!((last.x() - 100.0).abs() < 1e-9 && last.y().abs() < 1e-9);
    }

    #[test]
    fn malformed_or_unstarted_paths_are_refused() {
        assert!(matches!(
            contours_from_svg_path("M0 0 L1"),
            Err(SvgPathError::Parse(_))
        ));
        assert!(matches!(
            contours_from_svg_path("L10 10"),
            Err(SvgPathError::Parse(_) | SvgPathError::MissingMoveTo)
        ));
    }

    #[test]
    fn empty_path_data_has_no_contours() {
        assert!(contours_from_svg_path("").unwrap().is_empty());
    }
}
