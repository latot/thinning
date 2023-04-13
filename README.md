Adaptation of <https://github.com/OSGeo/grass/blob/main/raster/r.thin/thin_lines.c> for large images.

Do not run on `NBITS=1` images!

Compile with `cargo build --release`, then run it with `target/release/thinning <image.tif>`.
It overwrites the input image!
