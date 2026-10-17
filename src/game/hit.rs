/// A shape for simple collision tests: a rectangle or a circle.
///
/// Positions are in whatever units your game uses, usually cells. A
/// rectangle is given by its top-left corner and its size. A circle is given
/// by its centre and its radius. Two shapes overlap when they share some
/// area; shapes that only touch at an edge do not overlap:
///
/// ```
/// use terminal_graphics_interactive::game::Hit;
///
/// let player = Hit::rect(10.0, 5.0, 4.0, 2.0);
/// let coin = Hit::circle(12.0, 6.0, 1.0);
/// if player.overlaps(&coin) {
///     // collect the coin
/// }
/// assert!(player.overlaps(&coin));
/// assert!(!Hit::rect(0.0, 0.0, 2.0, 2.0).overlaps(&Hit::rect(2.0, 0.0, 2.0, 2.0)));
/// ```
///
/// A terminal cell is about twice as tall as it is wide, so a circle drawn
/// in cells looks like a tall oval. For round-looking hit circles, measure
/// `x` in half cells (multiply columns by 0.5), or use [`Hit::rect`]:
///
/// ```
/// use terminal_graphics_interactive::game::Hit;
///
/// let (col, row) = (20.0_f32, 8.0_f32);
/// let round_ball = Hit::circle(col * 0.5, row, 2.0);
/// assert!(round_ball.contains(10.5, 8.0));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    /// A rectangle with its top-left corner at `(x, y)`.
    Rect {
        /// Left edge.
        x: f32,
        /// Top edge.
        y: f32,
        /// Width, never negative.
        w: f32,
        /// Height, never negative.
        h: f32,
    },
    /// A circle with its centre at `(x, y)`.
    Circle {
        /// Centre, horizontal.
        x: f32,
        /// Centre, vertical.
        y: f32,
        /// Radius, never negative.
        r: f32,
    },
}

impl Hit {
    /// A rectangle with its top-left corner at `(x, y)`. A negative width or
    /// height counts as zero, which is an empty shape.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hit;
    ///
    /// let wall = Hit::rect(0.0, 0.0, 10.0, 4.0);
    /// assert!(wall.contains(9.0, 3.0));
    /// ```
    pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Hit {
        Hit::Rect {
            x,
            y,
            w: size(w),
            h: size(h),
        }
    }

    /// A circle centred on `(x, y)`. A negative radius counts as zero.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hit;
    ///
    /// let coin = Hit::circle(3.0, 3.0, 1.5);
    /// assert!(coin.overlaps(&Hit::rect(3.0, 2.0, 4.0, 4.0)));
    /// ```
    pub fn circle(x: f32, y: f32, r: f32) -> Hit {
        Hit::Circle { x, y, r: size(r) }
    }

    /// The same shape moved by `(dx, dy)`. Use it to test a shape where it
    /// will be after this step: `player.moved(speed, 0.0)`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hit;
    ///
    /// let coin = Hit::circle(10.0, 4.0, 1.0);
    /// let next = coin.moved(-2.0, 0.0);
    /// assert_eq!(next, Hit::circle(8.0, 4.0, 1.0));
    /// ```
    pub fn moved(&self, dx: f32, dy: f32) -> Hit {
        match *self {
            Hit::Rect { x, y, w, h } => Hit::Rect {
                x: x + dx,
                y: y + dy,
                w,
                h,
            },
            Hit::Circle { x, y, r } => Hit::Circle {
                x: x + dx,
                y: y + dy,
                r,
            },
        }
    }

    /// True if the point `(x, y)` is inside the shape. A rectangle includes
    /// its left and top edges but not its right and bottom ones, so a grid of
    /// touching rectangles has no gaps and no double hits. A circle excludes
    /// its edge.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hit;
    ///
    /// let button = Hit::rect(10.0, 4.0, 8.0, 2.0);
    /// // a mouse click at column 12, row 5
    /// assert!(button.contains(12.0, 5.0));
    /// ```
    pub fn contains(&self, x: f32, y: f32) -> bool {
        match *self {
            Hit::Rect { x: rx, y: ry, w, h } => {
                x >= rx && x < rx + size(w) && y >= ry && y < ry + size(h)
            }
            Hit::Circle { x: cx, y: cy, r } => {
                let r = size(r);
                let (dx, dy) = (x - cx, y - cy);
                dx * dx + dy * dy < r * r
            }
        }
    }

    /// True if the two shapes share some area. Shapes that only touch at an
    /// edge or a point do not overlap, and an empty shape (zero size or
    /// radius) overlaps nothing. A NaN coordinate never overlaps.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hit;
    ///
    /// let a = Hit::circle(0.0, 0.0, 2.0);
    /// let b = Hit::circle(3.0, 0.0, 2.0);
    /// assert!(a.overlaps(&b));
    /// assert!(!a.overlaps(&Hit::circle(5.0, 0.0, 2.0)));
    /// ```
    pub fn overlaps(&self, other: &Hit) -> bool {
        match (*self, *other) {
            (
                Hit::Rect {
                    x: ax,
                    y: ay,
                    w: aw,
                    h: ah,
                },
                Hit::Rect {
                    x: bx,
                    y: by,
                    w: bw,
                    h: bh,
                },
            ) => {
                let a_has_area = size(aw) > 0.0 && size(ah) > 0.0;
                let b_has_area = size(bw) > 0.0 && size(bh) > 0.0;
                a_has_area
                    && b_has_area
                    && ax < bx + size(bw)
                    && bx < ax + size(aw)
                    && ay < by + size(bh)
                    && by < ay + size(ah)
            }
            (
                Hit::Circle {
                    x: ax,
                    y: ay,
                    r: ar,
                },
                Hit::Circle {
                    x: bx,
                    y: by,
                    r: br,
                },
            ) => {
                let (ar, br) = (size(ar), size(br));
                let (dx, dy) = (ax - bx, ay - by);
                let reach = ar + br;
                ar > 0.0 && br > 0.0 && dx * dx + dy * dy < reach * reach
            }
            (Hit::Rect { .. }, Hit::Circle { .. }) => rect_circle(self, other),
            (Hit::Circle { .. }, Hit::Rect { .. }) => rect_circle(other, self),
        }
    }
}

/// Rect against circle: find the point of the rect nearest the circle's
/// centre, then ask whether that point is inside the circle.
fn rect_circle(rect: &Hit, circle: &Hit) -> bool {
    let (Hit::Rect { x, y, w, h }, Hit::Circle { x: cx, y: cy, r }) = (*rect, *circle) else {
        return false;
    };
    let (w, h, r) = (size(w), size(h), size(r));
    if !(w > 0.0 && h > 0.0 && r > 0.0) {
        return false;
    }
    let nearest_x = clamp_to(cx, x, x + w);
    let nearest_y = clamp_to(cy, y, y + h);
    let (dx, dy) = (cx - nearest_x, cy - nearest_y);
    dx * dx + dy * dy < r * r
}

/// `value` held within `lo..=hi`. Unlike `f32::clamp` this never panics, and
/// a NaN value passes through unchanged (and then never overlaps).
fn clamp_to(value: f32, lo: f32, hi: f32) -> f32 {
    if value < lo {
        lo
    } else if value > hi {
        hi
    } else {
        value
    }
}

/// A size or radius with negatives and NaN turned into zero.
fn size(value: f32) -> f32 {
    if value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_rects_overlap() {
        let a = Hit::rect(0.0, 0.0, 10.0, 10.0);
        let b = Hit::rect(5.0, 5.0, 10.0, 10.0);
        assert!(a.overlaps(&b));
        assert!(!Hit::rect(0.0, 0.0, 2.0, 2.0).overlaps(&Hit::rect(5.0, 5.0, 2.0, 2.0)));
    }

    #[test]
    fn circles_overlap_when_they_are_closer_than_the_sum_of_radii() {
        let a = Hit::circle(0.0, 0.0, 5.0);
        assert!(a.overlaps(&Hit::circle(6.0, 0.0, 2.0)));
        assert!(!a.overlaps(&Hit::circle(8.0, 0.0, 2.0)), "apart");
        assert!(!a.overlaps(&Hit::circle(7.0, 0.0, 2.0)), "touching");
    }

    #[test]
    fn a_circle_overlaps_a_rect_by_its_nearest_point_not_its_centre() {
        // A long, thin rect. The circle's centre is 2.5 below the rect's
        // middle, but only 2.0 below its nearest point, which is closer than
        // the radius 2.5.
        let strip = Hit::rect(0.0, 0.0, 100.0, 1.0);
        let ball = Hit::circle(50.0, 3.0, 2.5);
        assert!(strip.overlaps(&ball));
        assert!(ball.overlaps(&strip));
        assert!(
            !strip.overlaps(&Hit::circle(50.0, 3.5, 2.5)),
            "too far from the edge"
        );
    }

    #[test]
    fn a_shape_inside_another_overlaps_it() {
        let big = Hit::rect(0.0, 0.0, 10.0, 10.0);
        assert!(big.overlaps(&Hit::rect(2.0, 2.0, 1.0, 1.0)), "rect in rect");
        assert!(big.overlaps(&Hit::circle(5.0, 5.0, 1.0)), "circle in rect");
        assert!(
            Hit::circle(5.0, 5.0, 20.0).overlaps(&Hit::rect(2.0, 2.0, 1.0, 1.0)),
            "rect in circle"
        );
        assert!(
            Hit::circle(0.0, 0.0, 10.0).overlaps(&Hit::circle(0.0, 0.0, 1.0)),
            "circle in circle"
        );
    }

    #[test]
    fn a_circle_touching_a_rect_edge_does_not_overlap_it() {
        let rect = Hit::rect(0.0, 0.0, 10.0, 10.0);
        assert!(!rect.overlaps(&Hit::circle(12.0, 5.0, 2.0)));
        assert!(!Hit::circle(12.0, 5.0, 2.0).overlaps(&rect));
    }

    #[test]
    fn contains_counts_the_left_and_top_edge_of_a_rect_but_not_the_right_and_bottom() {
        let rect = Hit::rect(2.0, 3.0, 4.0, 2.0);
        assert!(rect.contains(2.0, 3.0));
        assert!(rect.contains(5.5, 4.5));
        assert!(
            !rect.contains(6.0, 4.0),
            "right edge belongs to the next cell"
        );
        assert!(
            !rect.contains(3.0, 5.0),
            "bottom edge belongs to the next row"
        );
        assert!(!rect.contains(1.9, 4.0));
    }

    #[test]
    fn contains_is_strictly_inside_a_circle() {
        let ball = Hit::circle(0.0, 0.0, 2.0);
        assert!(ball.contains(0.0, 0.0));
        assert!(ball.contains(1.0, 1.0));
        assert!(!ball.contains(2.0, 0.0), "on the edge is outside");
        assert!(!ball.contains(f32::NAN, 0.0));
    }

    #[test]
    fn moved_shifts_the_shape_and_keeps_its_size() {
        assert_eq!(
            Hit::rect(1.0, 2.0, 3.0, 4.0).moved(10.0, -1.0),
            Hit::rect(11.0, 1.0, 3.0, 4.0)
        );
        assert_eq!(
            Hit::circle(0.0, 0.0, 2.0).moved(1.5, 2.5),
            Hit::circle(1.5, 2.5, 2.0)
        );
    }

    #[test]
    fn negative_sizes_count_as_empty_and_nan_never_overlaps() {
        let wall = Hit::rect(0.0, 0.0, 10.0, 10.0);
        assert!(!wall.overlaps(&Hit::rect(1.0, 1.0, -5.0, 2.0)));
        assert!(!wall.overlaps(&Hit::circle(5.0, 5.0, -1.0)));
        assert!(!Hit::circle(5.0, 5.0, -1.0).contains(5.0, 5.0));
        assert!(!wall.contains(f32::NAN, 1.0));
        assert!(!wall.overlaps(&Hit::rect(f32::NAN, 1.0, 2.0, 2.0)));
        assert!(!wall.overlaps(&Hit::circle(f32::NAN, 5.0, 2.0)));
        assert!(!Hit::circle(f32::NAN, 0.0, 1.0).overlaps(&Hit::circle(0.0, 0.0, 1.0)));
        // Built directly, without the constructor, a negative size is still safe.
        let raw = Hit::Rect {
            x: 0.0,
            y: 0.0,
            w: -4.0,
            h: 4.0,
        };
        assert!(!raw.overlaps(&wall));
        assert!(!raw.contains(-1.0, 1.0));
    }

    fn random_shape(rng: &mut crate::game::Rng) -> Hit {
        let (x, y) = (rng.range_f32(-20.0, 20.0), rng.range_f32(-20.0, 20.0));
        let size = rng.range_f32(-2.0, 12.0);
        if rng.chance(0.5) {
            Hit::rect(x, y, size, rng.range_f32(-2.0, 12.0))
        } else {
            Hit::circle(x, y, size)
        }
    }

    #[test]
    fn overlap_is_symmetric_over_many_random_pairs() {
        let mut rng = crate::game::Rng::new(2026);
        let mut overlapping = 0;
        for _ in 0..5_000 {
            let a = random_shape(&mut rng);
            let b = random_shape(&mut rng);
            assert_eq!(a.overlaps(&b), b.overlaps(&a), "{a:?} vs {b:?}");
            if a.overlaps(&b) {
                overlapping += 1;
            }
        }
        // Make sure the test really sees both answers.
        assert!(
            (200..4_800).contains(&overlapping),
            "overlapping pairs: {overlapping}"
        );
    }

    #[test]
    fn rects_that_only_touch_at_an_edge_do_not_overlap() {
        let a = Hit::rect(0.0, 0.0, 10.0, 10.0);
        assert!(!a.overlaps(&Hit::rect(10.0, 0.0, 5.0, 5.0)), "right edge");
        assert!(!a.overlaps(&Hit::rect(0.0, 10.0, 5.0, 5.0)), "bottom edge");
        assert!(!a.overlaps(&Hit::rect(10.0, 10.0, 5.0, 5.0)), "corner");
    }
}
