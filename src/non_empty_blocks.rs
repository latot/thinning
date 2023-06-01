use std::{
    ffi::{c_char, CStr},
    sync::mpsc,
    thread,
};

use anyhow::Result;
use gdal::{errors::GdalError, raster::RasterBand, Dataset};
use gdal_sys::CPLErr;

pub fn non_empty_blocks(path: &String, num_threads: usize) -> Result<Vec<u8>> {
    //let path = "/home/pipe/raster.tif";
    let ds = Dataset::open(path.clone())?;
    let band = ds.rasterband(1)?;
    //let size = band.size();
    //let block_size = band.block_size();
    let (tile_width, tile_height) = band.block_size();
    let (width, height) = band.size();
    let (blocks_x, blocks_y) = (
        (width + tile_width - 1) / tile_width,
        (height + tile_width - 1) / tile_height,
    );
    let ntx = (width + tile_width - 1) / tile_width;
    let nty = (height + tile_height - 1) / tile_height;
    let total_tiles = ntx * nty;
    dbg!((blocks_x, blocks_y));
    // let mut buf = vec![0u8; block_size.0 * block_size.1];
    // for y in 0..blocks_y {
    //     for x in 0..blocks_x {
    //         band.read_block(x, y, &mut buf)?;
    //         let (w, h) = band.actual_block_size((x as isize, y as isize))?;
    //         let valid = &buf[..w * h];
    //         let has_data = valid.iter().any(|&x| x != 0);
    //         if has_data {
    //             println!("({y}, {x})");
    //         }
    //     }
    //     // println!("{y}/{blocks_y}");
    // }

    //let num_threads = 14;
    let mut threads = Vec::new();
    let (tx, rx) = mpsc::sync_channel(128);

    for id in 0..num_threads {
        let path = path.clone();
        let tx = tx.clone();
        let thread = thread::spawn(move || -> Result<()> {
            let ds = Dataset::open(path)?;
            let band = ds.rasterband(1)?;
            let size = band.size();
            let block_size = band.block_size();
            let (blocks_x, blocks_y) = (
                (size.0 + block_size.0 - 1) / block_size.0,
                (size.1 + block_size.1 - 1) / block_size.1,
            );
            let mut buf = vec![0u8; block_size.0 * block_size.1];
            let mut block_id = 0;
            for y in 0..blocks_y {
                for x in 0..blocks_x {
                    if block_id == id {
                        band.read_block(x, y, &mut buf)?;
                        let (w, h) = band.actual_block_size((x as isize, y as isize))?;
                        let valid = &buf[..w * h];
                        let has_data = valid.iter().any(|&x| (x & 1) != 0);
                        if has_data {
                            tx.send(Some((y, x))).unwrap();
                        }
                    }
                    block_id += 1;
                    if block_id == num_threads {
                        block_id = 0;
                    }
                }
            }

            tx.send(None).unwrap();
            Ok(())
        });
        threads.push(thread);
    }

    let mut remaining = num_threads;
    //All the tiles will be ready
    let mut ret = vec![1u8; total_tiles];
    for m in rx {
        match m {
            Some((y, x)) => {
                //println!("({y}, {x})");
                //If the tile has data, means is not ready, or at least need
                //the algorithm to start there
                ret[y*ntx + x] = 0;
            }
            None => remaining -= 1,
        }
        if remaining == 0 {
            break;
        }
    }

    for thread in threads {
        thread.join().unwrap().unwrap();
    }

    Ok(ret)
}

trait RasterBandExt {
    fn read_block<T>(&self, x: usize, y: usize, buf: &mut [T]) -> gdal::errors::Result<()>;
}

impl RasterBandExt for RasterBand<'_> {
    fn read_block<T>(&self, x: usize, y: usize, buf: &mut [T]) -> gdal::errors::Result<()> {
        let rv = unsafe {
            gdal_sys::GDALReadBlock(
                self.c_rasterband(),
                x as i32,
                y as i32,
                buf.as_mut_ptr() as *mut _,
            )
        };
        if rv != CPLErr::CE_None {
            return Err(_last_cpl_err(rv));
        }
        Ok(())
    }
}

pub fn _last_cpl_err(cpl_err_class: CPLErr::Type) -> GdalError {
    let last_err_no = unsafe { gdal_sys::CPLGetLastErrorNo() };
    let last_err_msg = _string(unsafe { gdal_sys::CPLGetLastErrorMsg() });
    unsafe { gdal_sys::CPLErrorReset() };
    GdalError::CplError {
        class: cpl_err_class,
        number: last_err_no,
        msg: last_err_msg,
    }
}

pub fn _string(raw_ptr: *const c_char) -> String {
    let c_str = unsafe { CStr::from_ptr(raw_ptr) };
    c_str.to_string_lossy().into_owned()
}