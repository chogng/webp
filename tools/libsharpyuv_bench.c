#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>

#include "sharpyuv/sharpyuv.h"
#include "sharpyuv/sharpyuv_csp.h"
#include "src/dsp/cpu.h"

typedef struct {
  uint8_t* rgba;
  int width;
  int height;
  size_t rgba_size;
} Image;

extern void SharpYuvInit(VP8CPUInfo cpu_info_func);

static double NowMilliseconds(void) {
  struct timeval value;
  gettimeofday(&value, NULL);
  return 1000.0 * (double)value.tv_sec + (double)value.tv_usec / 1000.0;
}

static uint32_t ReadU32(FILE* file, int* ok) {
  uint8_t bytes[4];
  if (fread(bytes, 1, sizeof(bytes), file) != sizeof(bytes)) {
    *ok = 0;
    return 0;
  }
  return (uint32_t)bytes[0] | ((uint32_t)bytes[1] << 8) |
         ((uint32_t)bytes[2] << 16) | ((uint32_t)bytes[3] << 24);
}

static uint64_t ReadU64(FILE* file, int* ok) {
  const uint64_t low = ReadU32(file, ok);
  const uint64_t high = ReadU32(file, ok);
  return low | (high << 32);
}

static Image* ReadCorpus(const char* path, int* count) {
  FILE* file = fopen(path, "rb");
  char magic[8];
  int ok = 1;
  if (file == NULL || fread(magic, 1, sizeof(magic), file) != sizeof(magic) ||
      memcmp(magic, "SYUVRGBA", sizeof(magic)) != 0) {
    if (file != NULL) fclose(file);
    return NULL;
  }
  const uint32_t image_count = ReadU32(file, &ok);
  Image* images = (Image*)calloc(image_count, sizeof(*images));
  if (!ok || image_count == 0 || images == NULL) {
    free(images);
    fclose(file);
    return NULL;
  }
  for (uint32_t i = 0; i < image_count; ++i) {
    images[i].width = (int)ReadU32(file, &ok);
    images[i].height = (int)ReadU32(file, &ok);
    const uint64_t rgba_size = ReadU64(file, &ok);
    const uint64_t expected =
        4u * (uint64_t)images[i].width * (uint64_t)images[i].height;
    if (!ok || rgba_size != expected || rgba_size != (size_t)rgba_size) {
      ok = 0;
      break;
    }
    images[i].rgba_size = (size_t)rgba_size;
    images[i].rgba = (uint8_t*)malloc(images[i].rgba_size);
    if (images[i].rgba == NULL ||
        fread(images[i].rgba, 1, images[i].rgba_size, file) !=
            images[i].rgba_size) {
      ok = 0;
      break;
    }
  }
  fclose(file);
  if (!ok) {
    for (uint32_t i = 0; i < image_count; ++i) free(images[i].rgba);
    free(images);
    return NULL;
  }
  *count = (int)image_count;
  return images;
}

static uint64_t HashBytes(uint64_t checksum, const uint8_t* bytes, size_t size) {
  for (size_t i = 0; i < size; ++i) {
    checksum ^= bytes[i];
    checksum *= UINT64_C(1099511628211);
  }
  return checksum;
}

static uint64_t VisibleChecksum(const Image* image) {
  const int uv_width = (image->width + 1) / 2;
  const int uv_height = (image->height + 1) / 2;
  const size_t y_size = (size_t)image->width * (size_t)image->height;
  const size_t uv_size = (size_t)uv_width * (size_t)uv_height;
  uint8_t* const y = (uint8_t*)malloc(y_size);
  uint8_t* const u = (uint8_t*)malloc(uv_size);
  uint8_t* const v = (uint8_t*)malloc(uv_size);
  if (y == NULL || u == NULL || v == NULL ||
      !SharpYuvConvert(image->rgba, image->rgba + 1, image->rgba + 2, 4,
                       image->width * 4, 8, y, image->width, u, uv_width, v,
                       uv_width, 8, image->width, image->height,
                       SharpYuvGetConversionMatrix(kSharpYuvMatrixWebp))) {
    free(y);
    free(u);
    free(v);
    return 0;
  }
  uint64_t checksum = UINT64_C(14695981039346656037);
  const uint64_t width = (uint64_t)image->width;
  const uint64_t height = (uint64_t)image->height;
  checksum = HashBytes(checksum, (const uint8_t*)&width, sizeof(width));
  checksum = HashBytes(checksum, (const uint8_t*)&height, sizeof(height));
  checksum = HashBytes(checksum, y, y_size);
  checksum = HashBytes(checksum, u, uv_size);
  checksum = HashBytes(checksum, v, uv_size);
  free(y);
  free(u);
  free(v);
  return checksum;
}

int main(int argc, char** argv) {
  if (argc != 4 || (strcmp(argv[1], "simd") != 0 &&
                   strcmp(argv[1], "scalar") != 0)) {
    fprintf(stderr,
            "usage: libsharpyuv_bench <simd|scalar> <iterations> <rgba-corpus>\n");
    return 1;
  }
  if (strcmp(argv[1], "scalar") == 0) SharpYuvInit(NULL);
  const int iterations = atoi(argv[2]);
  if (iterations <= 0) return 1;
  int count = 0;
  Image* images = ReadCorpus(argv[3], &count);
  if (images == NULL) return 1;

  uint64_t checksum = 0;
  uint64_t source_checksum = 0;
  size_t rgba_bytes = 0;
  for (int i = 0; i < count; ++i) {
    source_checksum += HashBytes(UINT64_C(14695981039346656037), images[i].rgba,
                                 images[i].rgba_size);
  }
  const double started = NowMilliseconds();
  for (int iteration = 0; iteration < iterations; ++iteration) {
    for (int i = 0; i < count; ++i) {
      const uint64_t hash = VisibleChecksum(&images[i]);
      if (hash == 0) return 1;
      checksum += hash;
      rgba_bytes += images[i].rgba_size;
    }
  }
  const double elapsed = NowMilliseconds() - started;
  printf("component=libsharpyuv mode=%s files=%d conversions=%d "
         "rgba_bytes=%zu elapsed_ms=%.3f checksum=%llu source_checksum=%llu\n",
         argv[1], count, count * iterations, rgba_bytes, elapsed,
         (unsigned long long)checksum, (unsigned long long)source_checksum);
  for (int i = 0; i < count; ++i) free(images[i].rgba);
  free(images);
  return 0;
}
