//! Brand assets — app icon used by the native window + the web
//! favicon. Generated procedurally so we don't have to ship a PNG
//! and worry about distro / DPI scaling.
//!
//! [`app_icon_rgba`] returns a `(width, height, Vec<u8>)` tuple of
//! sRGBA pixels (top-down rows, 4 bytes per pixel). Suitable
//! direct input to `winit::window::Icon::from_rgba`.

/// Icon side length in pixels. 64×64 is a sweet spot between
/// taskbar legibility and quick generation (~16 KB buffer).
const SIDE: u32 = 64;

const BG: [u8; 4] = [0x0c, 0x52, 0x2c, 0xff]; // dark felt green
const CARD_BG: [u8; 4] = [0xff, 0xff, 0xff, 0xff]; // white card
const CARD_BORDER: [u8; 4] = [0x33, 0x33, 0x33, 0xff]; // outline
const HEART: [u8; 4] = [0xd0, 0x1a, 0x1a, 0xff]; // heart red

/// Generate the application icon. Returns `(width, height,
/// rgba)`. RGBA is in scan-line top-down order, 4 bytes per
/// pixel. The icon is a stylised playing card on a green felt
/// background with a red heart pip — recognisable as a solitaire
/// app at 16 px and up.
pub fn app_icon_rgba() -> (u32, u32, Vec<u8>) {
    let mut buf = vec![0u8; (SIDE * SIDE * 4) as usize];

    // ── Background ──────────────────────────────────────────
    fill_all(&mut buf, BG);

    // ── Card body ───────────────────────────────────────────
    // Card occupies most of the canvas with a small inset so the
    // felt frames it on every side. 4 px corner radius is plenty
    // at 64 px.
    let card_x = 10;
    let card_y = 6;
    let card_w = SIDE - card_x * 2;
    let card_h = SIDE - card_y * 2;
    let card_r = 5;
    fill_rounded_rect(&mut buf, card_x, card_y, card_w, card_h, card_r, CARD_BG);
    stroke_rounded_rect(
        &mut buf,
        card_x,
        card_y,
        card_w,
        card_h,
        card_r,
        CARD_BORDER,
    );

    // ── Heart pip (centre) ──────────────────────────────────
    // A heart shape from two overlapping circles plus a
    // downward-pointing triangle. Centred in the card.
    let cx = SIDE / 2;
    let cy = SIDE / 2;
    draw_heart(&mut buf, cx, cy + 2, 14, HEART);

    (SIDE, SIDE, buf)
}

fn put_pixel(buf: &mut [u8], x: u32, y: u32, rgba: [u8; 4]) {
    if x >= SIDE || y >= SIDE {
        return;
    }
    let i = ((y * SIDE + x) * 4) as usize;
    buf[i..i + 4].copy_from_slice(&rgba);
}

fn fill_all(buf: &mut [u8], rgba: [u8; 4]) {
    for y in 0..SIDE {
        for x in 0..SIDE {
            put_pixel(buf, x, y, rgba);
        }
    }
}

/// Fill a rectangle with rounded corners. Uses a simple
/// quarter-circle inclusion test at each corner.
fn fill_rounded_rect(buf: &mut [u8], x: u32, y: u32, w: u32, h: u32, r: u32, rgba: [u8; 4]) {
    for dy in 0..h {
        for dx in 0..w {
            if !corner_inside(dx, dy, w, h, r) {
                continue;
            }
            put_pixel(buf, x + dx, y + dy, rgba);
        }
    }
}

/// 1-px outline along the rounded rectangle. Walks the perimeter
/// using the same corner-inclusion test as [`fill_rounded_rect`]
/// but only marks pixels on the boundary.
fn stroke_rounded_rect(buf: &mut [u8], x: u32, y: u32, w: u32, h: u32, r: u32, rgba: [u8; 4]) {
    for dy in 0..h {
        for dx in 0..w {
            if !corner_inside(dx, dy, w, h, r) {
                continue;
            }
            let neighbour_out = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|(nx, ny)| {
                    let cx = dx as i32 + nx;
                    let cy = dy as i32 + ny;
                    if cx < 0 || cy < 0 || cx >= w as i32 || cy >= h as i32 {
                        return true;
                    }
                    !corner_inside(cx as u32, cy as u32, w, h, r)
                });
            if neighbour_out {
                put_pixel(buf, x + dx, y + dy, rgba);
            }
        }
    }
}

fn corner_inside(dx: u32, dy: u32, w: u32, h: u32, r: u32) -> bool {
    let (rx, ry, in_corner) = if dx < r && dy < r {
        (r - dx, r - dy, true)
    } else if dx >= w - r && dy < r {
        (dx - (w - r - 1), r - dy, true)
    } else if dx < r && dy >= h - r {
        (r - dx, dy - (h - r - 1), true)
    } else if dx >= w - r && dy >= h - r {
        (dx - (w - r - 1), dy - (h - r - 1), true)
    } else {
        (0, 0, false)
    };
    if !in_corner {
        return true;
    }
    rx * rx + ry * ry <= r * r
}

/// Draw a filled heart shape centred at `(cx, cy)` (top of the
/// dip between the two lobes). `size` is the lobe radius; the
/// total bounding box is roughly `4*size` wide and `3.5*size`
/// tall.
fn draw_heart(buf: &mut [u8], cx: u32, cy: u32, size: u32, rgba: [u8; 4]) {
    let size_i = size as i32;
    let lobe_dx = size_i; // horizontal offset of each lobe centre
    let lobe_dy = -size_i + 1; // lobes sit slightly above cy
    let tip_y = cy as i32 + (size_i * 5) / 3;
    // Bounding box scan.
    for y in (cy as i32 - size_i * 2)..=tip_y {
        for x in (cx as i32 - size_i * 2)..=(cx as i32 + size_i * 2) {
            if x < 0 || y < 0 {
                continue;
            }
            // Left lobe.
            let lx = cx as i32 - lobe_dx;
            let ly = cy as i32 + lobe_dy;
            let in_left = (x - lx).pow(2) + (y - ly).pow(2) <= size_i.pow(2);
            // Right lobe.
            let rx = cx as i32 + lobe_dx;
            let ry = cy as i32 + lobe_dy;
            let in_right = (x - rx).pow(2) + (y - ry).pow(2) <= size_i.pow(2);
            // Triangle below: apex at (cx, tip_y), base from
            // (cx - 2*size, cy) to (cx + 2*size, cy). Use a
            // linear-interpolation test.
            let in_tri = {
                let dy = (y - cy as i32) as f32;
                let total_h = (tip_y - cy as i32) as f32;
                if dy < 0.0 || dy > total_h {
                    false
                } else {
                    let half_w = (size_i * 2) as f32 * (1.0 - dy / total_h);
                    ((x - cx as i32) as f32).abs() <= half_w
                }
            };
            if in_left || in_right || in_tri {
                put_pixel(buf, x as u32, y as u32, rgba);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_expected_buffer_size() {
        let (w, h, buf) = app_icon_rgba();
        assert_eq!(w, SIDE);
        assert_eq!(h, SIDE);
        assert_eq!(buf.len(), (SIDE * SIDE * 4) as usize);
    }

    #[test]
    fn icon_has_non_uniform_content() {
        // Sanity: the icon paints multiple colours (background +
        // card + heart). If every pixel is the same colour the
        // generator broke.
        let (_, _, buf) = app_icon_rgba();
        let first: &[u8] = &buf[0..4];
        let mixed = buf.chunks_exact(4).any(|p| p != first);
        assert!(mixed, "icon should have multiple colours");
    }

    #[test]
    fn icon_corner_is_background() {
        // (0,0) should be the felt-green background — confirms
        // the rounded corner doesn't cover the canvas edge.
        let (_, _, buf) = app_icon_rgba();
        assert_eq!(&buf[0..4], &BG);
    }
}
