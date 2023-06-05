import osgeo.gdal
import os
import os.path
import shutil
import time
import subprocess

script_dir = os.path.dirname(os.path.realpath(__file__))
thin_dir = os.path.join(script_dir, "..")

main_dir = os.getcwd()

os.chdir(main_dir)

samples = os.path.join(script_dir, "samples")

os.makedirs(samples, exist_ok = True)

base_example = os.path.join(script_dir, "example.tif")
base_thin = os.path.join(script_dir, "thin.tif")

with rasterio.open(base_thin) as src:
  thin_data = src.read(1)

def test_gdal_options(name, options):
  raster_file = tempfile.NamedTemporaryFile(suffix=".tif")
  time_start = time.time()
  ds = osgeo.gdal.Translate(
    raster_file.name,
    osgeo.gdal.Open(base_example),
    options = options
  )
  print("{}: Raster creation {}s".format(name, time.time()-start_time))
  ds = None
  time_start = time.time()
  cmd = 'cargo run --release \'{file}\''.format(file = raster_file.name)
  process = subprocess.Popen(cmd, shell=True, stdout=subprocess.PIPE)
  process.wait()
  if process.returncode: raise NameError("Error execute thinning on {}".format(name))
  print("{}: Raster thinning {}s".format(name, time.time()-start_time))
  with rasterio.open(raster_file.open()) as dst:
    if not (dst.read(1) == thin_data).all(): raise NameError("The thining is not implemented correctly on: {}".format(name))
  raster_file.close()

test_gdal_options("SPARSE + NBITS + ZSTD + TILED", '-co TILED=YES -co COMPRESS=ZSTD -co NBITS=2 -co SPARSE_OK=YES')
#SPARSE + NBITS + ZSTD + TILED
#ds = osgeo.gdal.Translate(
#  os.path.join(sample, "SNZT.tif"),
#  osgeo.gdal.Open(base_example),
#  options = '-co TILED=YES -co COMPRESS=ZSTD -co NBITS=2 -co SPARSE_OK=YES'
#)
#ds = None


