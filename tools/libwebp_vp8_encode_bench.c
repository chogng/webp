#include <stdint.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/time.h>

#include "webp/decode.h"
#include "webp/encode.h"

typedef struct {
  uint8_t* rgba;
  int width;
  int height;
  size_t rgba_size;
} Image;

static double NowMilliseconds(void) {
  struct timeval value;
  gettimeofday(&value, NULL);
  return 1000.0 * (double)value.tv_sec + (double)value.tv_usec / 1000.0;
}

static uint8_t* ReadFile(const char* path, size_t* size) {
  FILE* file = fopen(path, "rb");
  if (file == NULL) return NULL;
  if (fseek(file, 0, SEEK_END) != 0) return NULL;
  const long length = ftell(file);
  if (length < 0 || fseek(file, 0, SEEK_SET) != 0) return NULL;
  uint8_t* bytes = (uint8_t*)malloc((size_t)length);
  if (bytes == NULL || fread(bytes, 1, (size_t)length, file) != (size_t)length) {
    free(bytes);
    fclose(file);
    return NULL;
  }
  fclose(file);
  *size = (size_t)length;
  return bytes;
}

int main(int argc, char** argv) {
  static const float qualities[] = {0.0f, 75.0f, 100.0f};
  if (argc < 3) {
    fprintf(stderr, "usage: libwebp_vp8_encode_bench <iterations> <files...>\n");
    return 1;
  }
  const int iterations = atoi(argv[1]);
  if (iterations <= 0) return 1;
  const int count = argc - 2;
  Image* images = (Image*)calloc((size_t)count, sizeof(*images));
  if (images == NULL) return 1;
  for (int i = 0; i < count; ++i) {
    size_t input_size = 0;
    uint8_t* input = ReadFile(argv[i + 2], &input_size);
    if (input == NULL) return 1;
    images[i].rgba = WebPDecodeRGBA(
        input, input_size, &images[i].width, &images[i].height);
    free(input);
    if (images[i].rgba == NULL) return 1;
    images[i].rgba_size =
        4u * (size_t)images[i].width * (size_t)images[i].height;
  }

  uint64_t checksum = 0;
  size_t rgba_bytes = 0;
  size_t output_bytes = 0;
  const double started = NowMilliseconds();
  for (int iteration = 0; iteration < iterations; ++iteration) {
    for (size_t quality = 0; quality < sizeof(qualities) / sizeof(qualities[0]); ++quality) {
      for (int i = 0; i < count; ++i) {
        uint8_t* output = NULL;
        const size_t size = WebPEncodeRGBA(
            images[i].rgba, images[i].width, images[i].height,
            images[i].width * 4, qualities[quality], &output);
        if (size == 0 || output == NULL) return 1;
        checksum += (uint64_t)size + output[0];
        rgba_bytes += images[i].rgba_size;
        output_bytes += size;
        WebPFree(output);
      }
    }
  }
  const double elapsed = NowMilliseconds() - started;
  size_t quality_bytes[3] = {0, 0, 0};
  uint64_t quality_sse[3] = {0, 0, 0};
  uint64_t rgb_samples = 0;
  for (size_t quality = 0; quality < sizeof(qualities) / sizeof(qualities[0]); ++quality) {
    for (int i = 0; i < count; ++i) {
      uint8_t* output = NULL;
      const size_t size = WebPEncodeRGBA(
          images[i].rgba, images[i].width, images[i].height,
          images[i].width * 4, qualities[quality], &output);
      if (size == 0 || output == NULL) return 1;
      int decoded_width = 0;
      int decoded_height = 0;
      uint8_t* decoded =
          WebPDecodeRGBA(output, size, &decoded_width, &decoded_height);
      WebPFree(output);
      if (decoded == NULL || decoded_width != images[i].width ||
          decoded_height != images[i].height) {
        WebPFree(decoded);
        return 1;
      }
      quality_bytes[quality] += size;
      const size_t pixels = (size_t)images[i].width * (size_t)images[i].height;
      for (size_t pixel = 0; pixel < pixels; ++pixel) {
        for (size_t channel = 0; channel < 3; ++channel) {
          const int difference =
              (int)images[i].rgba[4 * pixel + channel] -
              (int)decoded[4 * pixel + channel];
          quality_sse[quality] += (uint64_t)(difference * difference);
        }
      }
      if (quality == 0) rgb_samples += 3u * (uint64_t)pixels;
      WebPFree(decoded);
    }
  }
  double quality_psnr[3];
  for (size_t quality = 0; quality < 3; ++quality) {
    quality_psnr[quality] = quality_sse[quality] == 0
                                ? INFINITY
                                : 10.0 * log10(255.0 * 255.0 *
                                               (double)rgb_samples /
                                               (double)quality_sse[quality]);
  }
  printf("encoder=libwebp profile=default qualities=0,75,100 files=%d "
         "encodes=%d rgba_bytes=%zu output_bytes=%zu elapsed_ms=%.3f "
         "checksum=%llu quality_bytes=%zu,%zu,%zu rgb_sse=%llu,%llu,%llu "
         "rgb_psnr=%.3f,%.3f,%.3f\n",
         count, count * iterations * 3, rgba_bytes, output_bytes, elapsed,
         (unsigned long long)checksum, quality_bytes[0], quality_bytes[1],
         quality_bytes[2], (unsigned long long)quality_sse[0],
         (unsigned long long)quality_sse[1],
         (unsigned long long)quality_sse[2], quality_psnr[0], quality_psnr[1],
         quality_psnr[2]);
  for (int i = 0; i < count; ++i) WebPFree(images[i].rgba);
  free(images);
  return 0;
}
