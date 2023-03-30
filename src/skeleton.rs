const HORIZONTAL: u8 = 1;
const VERTICAL: u8 = 2;

//================================
// MAIN ALGORITHM
//================================

// check if a region has any white pixel
fn not_empty(im: &[u8], ww: usize, _hh: usize, x: usize, y: usize, w: usize, h: usize) -> bool {
    for i in y..y + h {
        for j in x..x + w {
            if im[i * ww + j] != 0 {
                return true;
            }
        }
    }
    return false;
}

/**merge ith fragment of second chunk to first chunk
 * @param c0   fragments from  first  chunk
 * @param c1i  ith fragment of second chunk
 * @param sx   (x or y) coordinate of the seam
 * @param isv  is vertical, not horizontal?
 * @param mode 2-bit flag,
 *             MSB = is matching the left (not right) end of the fragment from first  chunk
 *             LSB = is matching the right (not left) end of the fragment from second chunk
 * @return     matching successful?             
 */
fn merge_impl(
    c0: &mut Vec<Vec<[usize; 2]>>,
    c1: &mut Vec<Vec<[usize; 2]>>,
    i: usize,
    sx: usize,
    isv: bool,
    mode: u8,
) -> bool {
    let b0: bool = (mode >> 1 & 1) > 0;
    let b1: bool = (mode >> 0 & 1) > 0;
    let mut mj: Option<usize> = None;
    let mut md: i32 = 4;
    let p1: [usize; 2] = c1[i][if b1 { 0 } else { c1[i].len() - 1 }];

    if ((if isv { p1[1] } else { p1[0] }) as i32 - sx as i32).abs() > 0 {
        // not on the seam, skip
        return false;
    }

    // find the best match
    for j in 0..c0.len() {
        let p0: [usize; 2] = c0[j][if b0 { 0 } else { c0[j].len() - 1 }];
        if ((if isv { p0[1] } else { p0[0] }) as i32 - sx as i32).abs() > 1 {
            continue;
        }
        let d: i32 = ((if isv { p0[0] } else { p0[1] }) as i32
            - (if isv { p1[0] } else { p1[1] }) as i32)
            .abs();
        if d < md {
            mj = Some(j);
            md = d;
        }
    }

    if mj.is_some() {
        let j: usize = mj.unwrap();
        if b0 && b1 {
            c1[i].reverse();
            c0[j].splice(0..0, c1[i].clone());
        } else if !b0 && b1 {
            c0[j].extend(c1[i].clone());
        } else if b0 && !b1 {
            c0[j].splice(0..0, c1[i].clone());
        } else {
            c1[i].reverse();
            c0[j].extend(c1[i].clone());
        }
        c1[i].clear();
        c1.remove(i);
        return true;
    }
    return false;
}

/**merge fragments from two chunks
 * @param c0   fragments from first  chunk
 * @param c1   fragments from second chunk
 * @param sx   (x or y) coordinate of the seam
 * @param dr   merge direction, HORIZONTAL or VERTICAL?
 */
fn merge_frags(c0: &mut Vec<Vec<[usize; 2]>>, c1: &mut Vec<Vec<[usize; 2]>>, sx: usize, dr: u8) {
    if c0.len() == 0 {
        c0.extend(c1.clone());
        c1.clear();
        return;
    }
    if c1.len() == 0 {
        return;
    }
    for i in (0..c1.len()).rev() {
        if dr == HORIZONTAL {
            if merge_impl(c0, c1, i, sx, false, 1) {
                continue;
            };
            if merge_impl(c0, c1, i, sx, false, 3) {
                continue;
            };
            if merge_impl(c0, c1, i, sx, false, 0) {
                continue;
            };
            if merge_impl(c0, c1, i, sx, false, 2) {
                continue;
            };
        } else if dr == VERTICAL {
            if merge_impl(c0, c1, i, sx, true, 1) {
                continue;
            };
            if merge_impl(c0, c1, i, sx, true, 3) {
                continue;
            };
            if merge_impl(c0, c1, i, sx, true, 0) {
                continue;
            };
            if merge_impl(c0, c1, i, sx, true, 2) {
                continue;
            };
        }
    }
    c0.extend(c1.clone());
    c1.clear();
    return;
}

/**recursive bottom: turn chunk into polyline fragments;
 * look around on 4 edges of the chunk, and identify the "outgoing" pixels;
 * add segments connecting these pixels to center of chunk;
 * apply heuristics to adjust center of chunk
 *
 * @param x    left of   chunk
 * @param y    top of    chunk
 * @param w    width of  chunk
 * @param h    height of chunk
 * @return     the polyline fragments
 */
fn chunk_to_frags(
    im: &[u8],
    ww: usize,
    _hh: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) -> Vec<Vec<[usize; 2]>> {
    let mut frags: Vec<Vec<[usize; 2]>> = vec![];
    let mut on: bool = false;
    let mut li: usize = 0;
    let mut lj: usize = 0;
    for k in 0..h + h + w + w - 4 {
        let i: i32;
        let j: i32;
        if (k as i32) < (w as i32) {
            i = y as i32 + 0;
            j = x as i32 + k as i32;
        } else if (k as i32) < (w as i32 + h as i32 - 1) {
            i = y as i32 + k as i32 - w as i32 + 1;
            j = x as i32 + w as i32 - 1;
        } else if (k as i32) < (w as i32 + h as i32 + w as i32 - 2) {
            i = y as i32 + h as i32 - 1;
            j = x as i32 + w as i32 - (k as i32 - w as i32 - h as i32 + 3);
        } else {
            i = y as i32 + h as i32 - (k as i32 - w as i32 - h as i32 - w as i32 + 4);
            j = x as i32 + 0;
        }
        let i: usize = i as usize;
        let j: usize = j as usize;

        if im[i * ww + j] != 0 {
            // found an outgoing pixel
            if !on {
                // left side of stroke
                on = true;
                frags.push(vec![[j, i], [x + w / 2, y + h / 2]]);
            }
        } else {
            if on {
                // right side of stroke, average to get center of stroke
                let l = frags.len();
                frags[l - 1][0][0] = (frags[l - 1][0][0] + lj) / 2;
                frags[l - 1][0][1] = (frags[l - 1][0][1] + li) / 2;
                on = false
            }
        }
        li = i;
        lj = j;
    }
    if frags.len() == 2 {
        let f: Vec<[usize; 2]> = vec![frags[0][0], frags[1][0]];
        frags.clear();
        frags.push(f);
    } else if frags.len() > 2 {
        let mut ms: u8 = 0;
        let mut mi: i32 = -1;
        let mut mj: i32 = -1;
        // use convolution to find brightest blob
        for i in y + 1..y + h - 1 {
            for j in x + 1..x + w - 1 {
                let s: u8 = (im[i * ww - ww + j - 1])
                    + (im[i * ww - ww + j])
                    + (im[i * ww - ww + j - 1 + 1])
                    + (im[i * ww + j - 1])
                    + (im[i * ww + j])
                    + (im[i * ww + j + 1])
                    + (im[i * ww + ww + j - 1])
                    + (im[i * ww + ww + j])
                    + (im[i * ww + ww + j + 1]);
                if s > ms {
                    mi = i as i32;
                    mj = j as i32;
                    ms = s;
                } else if s == ms
                    && (j as i32 - (x + w / 2) as i32).abs() + (i as i32 - (y + h / 2) as i32).abs()
                        < (mj - (x + w / 2) as i32).abs() + (mi - (y + h / 2) as i32).abs()
                {
                    mi = i as i32;
                    mj = j as i32;
                    ms = s;
                }
            }
        }
        if mi != -1 {
            for i in 0..frags.len() {
                frags[i][1][0] = mj as usize;
                frags[i][1][1] = mi as usize;
            }
        }
    }
    return frags;
}

/**Trace skeleton from thinning result.
 * Algorithm:
 * 1. if chunk size is small enough, reach recursive bottom and turn it into segments
 * 2. attempt to split the chunk into 2 smaller chunks, either horizontall or vertically;
 *    find the best "seam" to carve along, and avoid possible degenerate cases
 * 3. recurse on each chunk, and merge their segments
 *
 * @param x       left of   chunk
 * @param y       top of    chunk
 * @param w       width of  chunk
 * @param h       height of chunk
 * @param iter    current iteration
 * @return        an array of polylines
*/
pub fn trace_skeleton(
    im: &[u8],
    ww: usize,
    hh: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    chunk_size: usize,
    max_iter: usize,
) -> Vec<Vec<[usize; 2]>> {
    // for i in y..y+h{for j in x..x+w{print!("{}",im[i*ww+j]);}println!();}

    if max_iter <= 0 {
        dbg!("no");
        return vec![];
    }
    if w <= chunk_size && h <= chunk_size {
        return chunk_to_frags(im, ww, hh, x, y, w, h);
    }
    let mut ms: usize = ww + hh;
    let mut mi: i32 = -1;
    let mut mj: i32 = -1;
    if h > chunk_size {
        for i in y + 3..y + h - 3 {
            if im[i * ww + x] > 0
                || im[(i - 1) * ww + x] > 0
                || im[i * ww + x + w - 1] > 0
                || im[(i - 1) * ww + x + w - 1] > 0
            {
                continue;
            }
            let mut s: usize = 0;
            for j in x..x + w {
                s += im[i * ww + j] as usize;
                s += im[(i - 1) * ww + j] as usize;
            }
            if s < ms {
                ms = s;
                mi = i as i32;
            } else if s == ms
                && (i as i32 - (y + h / 2) as i32).abs() < (mi as i32 - (y + h / 2) as i32).abs()
            {
                // if there is a draw (very common), we want the seam to be near the middle
                // to balance the divide and conquer tree
                ms = s;
                mi = i as i32;
            }
        }
    }
    if w > chunk_size {
        for j in x + 3..x + w - 3 {
            if im[ww * y + j] > 0
                || im[ww * (y + h) - ww + j] > 0
                || im[ww * y + j - 1] > 0
                || im[ww * (y + h) - ww + j - 1] > 0
            {
                continue;
            }
            let mut s: usize = 0;
            for i in y..y + h {
                s += im[i * ww + j] as usize;
                s += im[i * ww + j - 1] as usize;
            }
            if s < ms {
                ms = s;
                mi = -1;
                mj = j as i32;
            } else if s == ms
                && (j as i32 - (x + w / 2) as i32).abs() < (mj as i32 - (x + w / 2) as i32).abs()
            {
                ms = s;
                mi = -1;
                mj = j as i32;
            }
        }
    }
    let (mut l0, mut l1, mut l2, mut l3): (usize, usize, usize, usize) =
        (usize::max_value(), 0, 0, 0);
    let (mut r0, mut r1, mut r2, mut r3): (usize, usize, usize, usize) =
        (usize::max_value(), 0, 0, 0);
    let mut dr: u8 = 0;
    let mut sx: usize = 0;

    if h > chunk_size && mi != -1 {
        l0 = x;
        l1 = y;
        l2 = w;
        l3 = (mi as i32 - y as i32) as usize;
        r0 = x;
        r1 = mi as usize;
        r2 = w;
        r3 = (y as i32 + h as i32 - mi as i32) as usize;
        dr = VERTICAL;
        sx = mi as usize;
    } else if w > chunk_size && mj != -1 {
        l0 = x;
        l1 = y;
        l2 = (mj as i32 - x as i32) as usize;
        l3 = h;
        r0 = mj as usize;
        r1 = y;
        r2 = (x as i32 + w as i32 - mj as i32) as usize;
        r3 = h;
        dr = HORIZONTAL;
        sx = mj as usize;
    }
    let mut frags: Vec<Vec<[usize; 2]>> = vec![];
    if l0 != usize::max_value() && not_empty(im, ww, hh, l0, l1, l2, l3) {
        // if there are no white pixels, don't waste time
        merge_frags(
            &mut frags,
            &mut trace_skeleton(im, ww, hh, l0, l1, l2, l3, chunk_size, max_iter - 1),
            sx,
            dr,
        );
    }
    if r0 != usize::max_value() && not_empty(im, ww, hh, r0, r1, r2, r3) {
        // if there are no white pixels, don't waste time
        merge_frags(
            &mut frags,
            &mut trace_skeleton(im, ww, hh, r0, r1, r2, r3, chunk_size, max_iter - 1),
            sx,
            dr,
        );
    }
    if mi == -1 && mj == -1 {
        // splitting failed! do the recursive bottom instead
        return chunk_to_frags(im, ww, hh, x, y, w, h);
    }
    return frags;
}
