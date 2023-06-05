import osgeo.gdal
import os
import os.path
import shutil
import time
import subprocess
import rasterio
import tempfile
import numpy

base_example = "example.tif"
base_thin = "thin.tif"

with rasterio.open(base_thin) as src:
  thin_data = numpy.bitwise_and(src.read(1), 1)

def test_gdal_options(name, options):
  raster_file = tempfile.NamedTemporaryFile(suffix=".tif")
  time_start = time.time()
  ds = osgeo.gdal.Translate(
    raster_file.name,
    osgeo.gdal.Open(base_example),
    options = options
  )
  print("{}: Raster creation {}s".format(name, time.time()-time_start))
  ds = None
  time_start = time.time()
  cmd = 'cargo run --release \'{file}\''.format(file = raster_file.name)
  process = subprocess.run(cmd, shell=True, capture_output = True)
  if process.returncode: raise NameError("Error execute thinning on {}".format(name))
  print("{}: Raster thinning {}s".format(name, time.time()-time_start))
  with rasterio.open(raster_file.name) as dst:
    dst_data = numpy.bitwise_and(dst.read(1), 1)
  if not (dst_data == thin_data).all(): raise NameError("The thining is not implemented correctly on: {}".format(name))
  raster_file.close()

test_gdal_options("SPARSE + NBITS + ZSTD + TILED", '-co TILED=YES -co COMPRESS=ZSTD -co NBITS=2 -co SPARSE_OK=YES')
test_gdal_options("SPARSE + NBITS + ZSTD + NO TILED", '-co TILED=NO -co COMPRESS=ZSTD -co NBITS=2 -co SPARSE_OK=YES')
test_gdal_options("NO SPARSE + NBITS + ZSTD + TILED", '-co TILED=YES -co COMPRESS=ZSTD -co NBITS=2 -co SPARSE_OK=NO')
test_gdal_options("SPARSE + NBITS + ZSTD + TILED", '-co TILED=YES -co COMPRESS=ZSTD -co NBITS=2 -co SPARSE_OK=YES')

