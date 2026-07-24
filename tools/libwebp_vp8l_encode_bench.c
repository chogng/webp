#include <stdint.h>
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

static size_t EncodeLosslessExact(const Image* image, int level,
                                  uint8_t** output) {
  WebPConfig config;
  WebPPicture picture;
  WebPMemoryWriter writer;
  int ok;

  *output = NULL;
  if (!WebPConfigInit(&config) ||
      !WebPConfigLosslessPreset(&config, level) ||
      !WebPPictureInit(&picture)) {
    return 0;
  }
  config.exact = 1;
  config.thread_level = 0;
  picture.use_argb = 1;
  picture.width = image->width;
  picture.height = image->height;
  WebPMemoryWriterInit(&writer);
  picture.writer = WebPMemoryWrite;
  picture.custom_ptr = &writer;

  ok = WebPPictureImportRGBA(&picture, image->rgba, image->width * 4) &&
       WebPEncode(&config, &picture);
  WebPPictureFree(&picture);
  if (!ok) {
    WebPMemoryWriterClear(&writer);
    return 0;
  }
  *output = writer.mem;
  return writer.size;
}

int main(int argc, char** argv) {
  if (argc < 4) {
    fprintf(stderr,
            "usage: libwebp_vp8l_encode_bench "
            "<iterations> <level 0..9> <files...>\n");
    return 1;
  }
  const int iterations = atoi(argv[1]);
  const int level = atoi(argv[2]);
  if (iterations <= 0 || level < 0 || level > 9) return 1;
  const int count = argc - 3;
  Image* images = (Image*)calloc((size_t)count, sizeof(*images));
  if (images == NULL) return 1;
  for (int i = 0; i < count; ++i) {
    size_t input_size = 0;
    uint8_t* input = ReadFile(argv[i + 3], &input_size);
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
    for (int i = 0; i < count; ++i) {
      uint8_t* output = NULL;
      const size_t size = EncodeLosslessExact(&images[i], level, &output);
      if (size == 0 || output == NULL) return 1;
      checksum += (uint64_t)size + output[0];
      rgba_bytes += images[i].rgba_size;
      output_bytes += size;
      WebPFree(output);
    }
  }
  const double elapsed = NowMilliseconds() - started;
  printf("encoder=libwebp profile=lossless-level-%d level=%d exact=1 "
         "files=%d encodes=%d "
         "rgba_bytes=%zu output_bytes=%zu elapsed_ms=%.3f checksum=%llu\n",
         level, level, count, count * iterations, rgba_bytes, output_bytes,
         elapsed,
         (unsigned long long)checksum);
  for (int i = 0; i < count; ++i) WebPFree(images[i].rgba);
  free(images);
  return 0;
}
