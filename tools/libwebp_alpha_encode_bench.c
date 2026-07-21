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
  if (file == NULL || fseek(file, 0, SEEK_END) != 0) return NULL;
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

static int Encode(const Image* image, WebPMemoryWriter* writer) {
  WebPConfig config;
  WebPPicture picture;
  if (!WebPConfigInit(&config) || !WebPPictureInit(&picture)) return 0;
  config.quality = 75.f;
  config.alpha_compression = 1;
  config.alpha_filtering = 1;
  config.alpha_quality = 100;
  picture.width = image->width;
  picture.height = image->height;
  picture.writer = WebPMemoryWrite;
  picture.custom_ptr = writer;
  if (!WebPPictureImportRGBA(&picture, image->rgba, image->width * 4)) return 0;
  const int ok = WebPEncode(&config, &picture);
  WebPPictureFree(&picture);
  return ok;
}

static uint32_t ReadLittleEndian32(const uint8_t* data) {
  return (uint32_t)data[0] | ((uint32_t)data[1] << 8) |
         ((uint32_t)data[2] << 16) | ((uint32_t)data[3] << 24);
}

static size_t AlphaPayloadSize(const uint8_t* data, size_t size) {
  size_t offset = 12;
  while (offset + 8 <= size) {
    const uint32_t chunk_size = ReadLittleEndian32(data + offset + 4);
    if (data[offset] == 'A' && data[offset + 1] == 'L' &&
        data[offset + 2] == 'P' && data[offset + 3] == 'H') {
      return chunk_size;
    }
    offset += 8u + chunk_size + (chunk_size & 1u);
  }
  return 0;
}

int main(int argc, char** argv) {
  if (argc < 3) {
    fprintf(stderr, "usage: libwebp_alpha_encode_bench <iterations> <files...>\n");
    return 1;
  }
  const int iterations = atoi(argv[1]);
  const int count = argc - 2;
  if (iterations <= 0) return 1;
  Image* images = (Image*)calloc((size_t)count, sizeof(*images));
  if (images == NULL) return 1;
  for (int i = 0; i < count; ++i) {
    size_t input_size = 0;
    uint8_t* input = ReadFile(argv[i + 2], &input_size);
    if (input == NULL) return 1;
    images[i].rgba = WebPDecodeRGBA(input, input_size, &images[i].width,
                                    &images[i].height);
    free(input);
    if (images[i].rgba == NULL) return 1;
    images[i].rgba_size =
        4u * (size_t)images[i].width * (size_t)images[i].height;
  }

  uint64_t checksum = 0;
  size_t rgba_bytes = 0;
  size_t output_bytes = 0;
  size_t alpha_bytes = 0;
  const double started = NowMilliseconds();
  for (int iteration = 0; iteration < iterations; ++iteration) {
    for (int i = 0; i < count; ++i) {
      WebPMemoryWriter writer;
      WebPMemoryWriterInit(&writer);
      if (!Encode(&images[i], &writer)) return 1;
      checksum += (uint64_t)writer.size + writer.mem[0];
      rgba_bytes += images[i].rgba_size;
      output_bytes += writer.size;
      alpha_bytes += AlphaPayloadSize(writer.mem, writer.size);
      WebPMemoryWriterClear(&writer);
    }
  }
  printf("encoder=libwebp profile=vp8-q75-alpha-lossless-fast files=%d "
         "encodes=%d rgba_bytes=%zu output_bytes=%zu alpha_bytes=%zu elapsed_ms=%.3f "
         "checksum=%llu\n",
         count, count * iterations, rgba_bytes, output_bytes, alpha_bytes,
         NowMilliseconds() - started, (unsigned long long)checksum);
  for (int i = 0; i < count; ++i) WebPFree(images[i].rgba);
  free(images);
  return 0;
}
