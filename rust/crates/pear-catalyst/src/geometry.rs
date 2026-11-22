//! Touch/drawable geometry shared by input and graphics (PearTouchGeometry.h).

use pear_objc::{CGPoint, CGRect, CGSize};

/// A fixed-orientation GL app can create its first drawable with UIKit's
/// stale opposite orientation: keep its dimensions, but respect an
/// unambiguous app declaration.
pub fn orient_drawable_size(size: CGSize, landscape: bool, portrait: bool) -> CGSize {
    if size.width <= 0.0 || size.height <= 0.0 || landscape == portrait {
        return size;
    }
    if (landscape && size.width < size.height) || (portrait && size.width > size.height) {
        return CGSize { width: size.height, height: size.width };
    }
    size
}

/// Centers content in a container without cropping or changing its aspect.
pub fn aspect_fit_rect(content: CGSize, container: CGRect) -> CGRect {
    if content.width <= 0.0 || content.height <= 0.0 || container.size.width <= 0.0 || container.size.height <= 0.0 {
        return CGRect::default();
    }
    let scale = (container.size.width / content.width).min(container.size.height / content.height);
    let fitted = CGSize { width: content.width * scale, height: content.height * scale };
    CGRect {
        origin: CGPoint {
            x: container.origin.x + container.size.width * 0.5 - fitted.width * 0.5,
            y: container.origin.y + container.size.height * 0.5 - fitted.height * 0.5,
        },
        size: fitted,
    }
}

/// Maps a point from the current aspect-fitted drawable to the window space
/// the game laid its controls out in; identity until the fit is applied.
pub fn map_fitted_touch(point: CGPoint, native: CGSize, reference: CGSize, current: CGSize, fit_applied: bool) -> CGPoint {
    if !fit_applied
        || native.width < 1.0
        || native.height < 1.0
        || reference.width < 1.0
        || reference.height < 1.0
        || current.width < 1.0
        || current.height < 1.0
    {
        return point;
    }
    let cs = (current.width / native.width).min(current.height / native.height);
    let rs = (reference.width / native.width).min(reference.height / native.height);
    if cs <= 0.0 || rs <= 0.0 {
        return point;
    }
    let co = CGPoint { x: (current.width - native.width * cs) * 0.5, y: (current.height - native.height * cs) * 0.5 };
    let ro = CGPoint { x: (reference.width - native.width * rs) * 0.5, y: (reference.height - native.height * rs) * 0.5 };
    CGPoint { x: ro.x + (point.x - co.x) * rs / cs, y: ro.y + (point.y - co.y) * rs / cs }
}

pub fn rect_is_empty(r: CGRect) -> bool {
    r.size.width <= 0.0 || r.size.height <= 0.0
}

pub fn point_in_rect(p: CGPoint, r: CGRect) -> bool {
    // NSPointInRect: half-open on the max edges.
    p.x >= r.origin.x && p.y >= r.origin.y && p.x < r.origin.x + r.size.width && p.y < r.origin.y + r.size.height
}
