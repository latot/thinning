use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    ptr, slice,
};

use gdal::{Dataset, DatasetOptions, GdalOpenFlags};
use gdal_sys::GDALRWFlag::GF_Write;
use indicatif::ProgressBar;
use log::LevelFilter;
use memmap2::{Mmap, MmapMut};

// Implementation of https://github.com/OSGeo/grass/blob/main/raster/r.thin/thin_lines.c

const TEMPL: [u8; 8] = [
40,
10,
130,
160,
42,
138,
162,
168
];

const N_TEMPL: [u8; 8] = [
131,
224,
56,
14,
128,
32,
8,
2
];

fn encode_neightbours(im: &[u8], i: usize, j: usize, w: usize, neg: bool) -> u8 {
  let p2: u8 = im[(i - 1) * w + j] & 1;
  let p3: u8 = im[(i - 1) * w + j + 1] & 1;
  let p4: u8 = im[(i) * w + j + 1] & 1;
  let p5: u8 = im[(i + 1) * w + j + 1] & 1;
  let p6: u8 = im[(i + 1) * w + j] & 1;
  let p7: u8 = im[(i + 1) * w + j - 1] & 1;
  let p8: u8 = im[(i) * w + j - 1] & 1;
  let p1: u8 = im[(i - 1) * w + j - 1] & 1;
  let t: u8;
  
  if neg {
    t =
      (!p6 << 5) |
      (!p7 << 6) |
      (!p8 << 7) |
      !p1 |
      (!p2 << 1) |
      (!p3 << 2) |
      (!p4 << 3) |
      (!p5 << 4);  
  } else {
    t =
      (p6 << 5) |
      (p7 << 6) |
      (p8 << 7) |
      p1 |
      (p2 << 1) |
      (p3 << 2) |
      (p4 << 3) |
      (p5 << 4);  
  }
  return t;
}

fn thinning_zs_iteration(
    im: &mut [u8],
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
    w: usize,
    h: usize
) -> bool {
    let mut diff: bool = false;
    let min_x = if win_x == 0 { 1 } else { win_x };
    let max_x = if win_x + win_w == w {
        w - 1
    } else {
        win_x + win_w
    };
    let min_y = if win_y == 0 { 1 } else { win_y };
    let max_y = if win_y + win_h == h {
        h - 1
    } else {
        win_y + win_h
    };
    for r in 1..5 {
      let ind1: usize;
      let ind2: usize;
      let ind3: usize;
      
      ind1 = r - 1;

      if r <= 3 {
        ind2 = r;
      } else {
        ind2 = 0;
      }

      ind3 = (r - 1) + 4;

      for i in min_y..max_y {
          for j in min_x..max_x {
              let p1: u8 = im[i * w + j] & 1;
              if p1 == 0 {
                  continue;
              }
              let w_: u8 = encode_neightbours(&im, i, j, w, true);
              let n_w_: u8 = encode_neightbours(&im, i, j, w, false);
              if
              (
                ((TEMPL[ind1] & w_) == TEMPL[ind1]) &&
                ((N_TEMPL[ind1] & n_w_) == N_TEMPL[ind1])
              ) ||
              (
                ((TEMPL[ind2] & w_) == TEMPL[ind2]) &&
                ((N_TEMPL[ind2] & n_w_) == N_TEMPL[ind2])
              ) ||
              (
                ((TEMPL[ind3] & w_) == TEMPL[ind3]) &&
                ((N_TEMPL[ind3] & n_w_) == N_TEMPL[ind3])
              )
              {
                  diff = true;
                  im[i * w + j] |= 2;
              }
          }
      }
    }
    return diff;
}

pub fn thinning_zs(im: &mut [u8], w: usize, h: usize) {
    let mut iter = 0;
    loop {
        dbg!(iter);
        let mut diff = false;
        if dbg!(thinning_zs_iteration(im, 0, 0, w, h, w, h)) {
            thinning_zs_post(im, 0, 0, w, h, w);
            diff = true;
        }
        if !diff {
            break;
        }
        iter += 1;
    }
}

fn thinning_zs_post(
    im: &mut [u8],
    win_x: usize,
    win_y: usize,
    win_w: usize,
    win_h: usize,
    w: usize,
) {
    for i in win_y..win_y + win_h {
        for j in win_x..win_x + win_w {
            let marker = im[i * w + j] >> 1;
            let old = im[i * w + j] & 1;
            let new = old & (!marker);
            if new != old {
                im[i * w + j] = new;
            }
        }
    }
}


pub fn thinning_zs_tiled(
    im: &mut [u8],
    width: usize,
    height: usize,
    tile_width: usize,
    tile_height: usize,
) {
    let ntx = (width + tile_width - 1) / tile_width;
    let nty = (height + tile_height - 1) / tile_height;
    let total_tiles = ntx * nty;

    const FLAG_NONE: u8 = 0;
    const FLAG_DONE: u8 = 1;
    const FLAG_CHANGED_H: u8 = 2;
    const FLAG_CHANGED_V: u8 = 4;
    let mut tile_flags = vec![FLAG_NONE; total_tiles];

    let mut iter = 1;
    loop {
        let remaining_tiles = tile_flags.iter().filter(|&f| f & FLAG_DONE == 0).count();
        if remaining_tiles == 0 {
            break;
        }
        let pb = ProgressBar::new(remaining_tiles as u64).with_message("Starting thinning H");
        log::info!("Starting iteration {iter}, {remaining_tiles}/{total_tiles}");
        log::info!("Starting thinning H");

        for ti_y in 0..nty {
            for ti_x in 0..ntx {
                if tile_flags[ti_y * ntx + ti_x] & FLAG_DONE != 0
                    && (ti_x == 0 || tile_flags[ti_y * ntx + ti_x - 1] & FLAG_DONE != 0)
                    && (ti_y == 0 || tile_flags[(ti_y - 1) * ntx + ti_x] & FLAG_DONE != 0)
                    && (ti_x == ntx - 1 || tile_flags[ti_y * ntx + ti_x + 1] & FLAG_DONE != 0)
                    && (ti_y == nty - 1 || tile_flags[(ti_y + 1) * ntx + ti_x] & FLAG_DONE != 0)
                {
                    continue;
                }
                let win_x = ti_x * tile_width;
                let win_y = ti_y * tile_height;
                let win_w = tile_width.min(width - win_x);
                let win_h = tile_height.min(height - win_y);
                if thinning_zs_iteration(im, win_x, win_y, win_w, win_h, width, height) {
                    tile_flags[ti_y * ntx + ti_x] |= FLAG_CHANGED_H;
                } else {
                    tile_flags[ti_y * ntx + ti_x] &= !FLAG_CHANGED_H;
                }
                pb.inc(1);
            }
        }
        pb.finish();

        let remaining_tiles = tile_flags
            .iter()
            .filter(|&f| f & FLAG_CHANGED_H != 0)
            .count();
        let pb = ProgressBar::new(remaining_tiles as u64).with_message("Starting pixel removal H");
        log::info!("Starting pixel removal H");
        for ti_y in 0..nty {
            for ti_x in 0..ntx {
                if tile_flags[ti_y * ntx + ti_x] & FLAG_CHANGED_H == 0 {
                    continue;
                }
                let win_x = ti_x * tile_width;
                let win_y = ti_y * tile_height;
                let win_w = tile_width.min(width - win_x);
                let win_h = tile_height.min(height - win_y);
                thinning_zs_post(im, win_x, win_y, win_w, win_h, width);
                pb.inc(1);
            }
        }
        pb.finish();

        let remaining_tiles = tile_flags.iter().filter(|&f| f & FLAG_DONE == 0).count();
        let pb = ProgressBar::new(remaining_tiles as u64).with_message("Starting thinning V");
        // thinning_zs_post(im, 0, 0, w, h, w);
        log::info!("Starting thinning V");
        for ti_y in 0..nty {
            for ti_x in 0..ntx {
                if tile_flags[ti_y * ntx + ti_x] & FLAG_DONE != 0
                    && (ti_x == 0 || tile_flags[ti_y * ntx + ti_x - 1] & FLAG_DONE != 0)
                    && (ti_y == 0 || tile_flags[(ti_y - 1) * ntx + ti_x] & FLAG_DONE != 0)
                    && (ti_x == ntx - 1 || tile_flags[ti_y * ntx + ti_x + 1] & FLAG_DONE != 0)
                    && (ti_y == nty - 1 || tile_flags[(ti_y + 1) * ntx + ti_x] & FLAG_DONE != 0)
                {
                    continue;
                }
                let win_x = ti_x * tile_width;
                let win_y = ti_y * tile_height;
                let win_w = tile_width.min(width - win_x);
                let win_h = tile_height.min(height - win_y);
                if thinning_zs_iteration(im, win_x, win_y, win_w, win_h, width, height) {
                    tile_flags[ti_y * ntx + ti_x] |= FLAG_CHANGED_V;
                } else {
                    tile_flags[ti_y * ntx + ti_x] &= !FLAG_CHANGED_V;
                }
                pb.inc(1);
            }
        }
        pb.finish();

        let remaining_tiles = tile_flags
            .iter()
            .filter(|&f| f & FLAG_CHANGED_V != 0)
            .count();
        let pb = ProgressBar::new(remaining_tiles as u64).with_message("Starting pixel removal V");
        log::info!("Starting pixel removal V");
        // thinning_zs_post(im, 0, 0, w, h, w);
        for ti_y in 0..nty {
            for ti_x in 0..ntx {
                if tile_flags[ti_y * ntx + ti_x] & FLAG_CHANGED_H == 0
                    && tile_flags[ti_y * ntx + ti_x] & FLAG_CHANGED_V == 0
                {
                    tile_flags[ti_y * ntx + ti_x] |= FLAG_DONE;
                    continue;
                }
                if tile_flags[ti_y * ntx + ti_x] & FLAG_CHANGED_V == 0 {
                    continue;
                }
                let win_x = ti_x * tile_width;
                let win_y = ti_y * tile_height;
                let win_w = tile_width.min(width - win_x);
                let win_h = tile_height.min(height - win_y);
                thinning_zs_post(im, win_x, win_y, win_w, win_h, width);
                pb.inc(1);
            }
        }
        pb.finish();

        iter += 1;
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let file = std::env::args().nth(1).unwrap();
    let mut ds = Dataset::open_ex(
        &file,
        DatasetOptions {
            open_flags: GdalOpenFlags::GDAL_OF_UPDATE,
            ..DatasetOptions::default()
        },
    )?;
    let band = ds.rasterband(1)?;
    let mut pixel_space = 0;
    let mut line_space = 0i64;
    let mem = unsafe {
        gdal_sys::GDALGetVirtualMemAuto(
            band.c_rasterband(),
            GF_Write,
            &mut pixel_space as *mut _,
            &mut line_space as *mut _,
            ptr::null::<i8>() as _,
        )
    };
    let (width, height) = band.size();
    assert_eq!(pixel_space, 1);
    assert_eq!(line_space, width as i64);
    let (tile_width, tile_height) = band.block_size();
    dbg!((width, height));
    dbg!((tile_width, tile_height));
    let data = unsafe { gdal_sys::CPLVirtualMemGetAddr(mem) } as *mut u8;
    let len = unsafe { gdal_sys::CPLVirtualMemGetSize(mem) };
    let im = unsafe { slice::from_raw_parts_mut(data, len) };

    // let file = OpenOptions::new().read(true).write(true).open(&file)?;
    // let mut im = unsafe { MmapMut::map_mut(&file)? };
    // let im = im.as_mut();

    let mut builder = env_logger::Builder::new();
    builder.filter_level(log::LevelFilter::Info);
    builder.parse_env("RUST_LOG");
    builder.init();

    // for i in 0..height * width {
    //     if im[i as usize] > 128 {
    //         im[i as usize] = 1
    //     } else {
    //         im[i as usize] = 0
    //     }
    // }

    // let width = 535120;
    // let height = 599280;

    thinning_zs(im, width, height);
    // thinning_zs_tiled(im, width, height, tile_width, tile_height);

    // let skeleton = skeleton::trace_skeleton(im, width, height, 0, 0, 100000, 100000, 10, 999);
    // let mut out = BufWriter::new(File::create("skeleton.csv")?);
    // for i in 0..skeleton.len() {
    //     for j in 0..skeleton[i].len() {
    //         write!(out, "{},{} ", skeleton[i][j][0], skeleton[i][j][1])?;
    //     }
    //     writeln!(out)?;
    // }

    unsafe { gdal_sys::CPLVirtualMemFree(mem) };

    ds.flush_cache();

    Ok(())
}
