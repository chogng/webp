#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>

#include "webp/decode.h"

typedef struct {
  uint8_t* bytes;
  size_t size;
} Input;

static double NowMilliseconds(void) {
  struct timeval value;
  gettimeofday(&value, NULL);
  return 1000.0 * (double)value.tv_sec + (double)value.tv_usec / 1000.0;
}

static int ReadFile(const char* path, Input* input) {
  FILE* file = fopen(path, "rb");
  if (file == NULL) {
    perror(path);
    return 0;
  }
  if (fseek(file, 0, SEEK_END) != 0) return 0;
  const long length = ftell(file);
  if (length < 0 || fseek(file, 0, SEEK_SET) != 0) return 0;
  input->bytes = (uint8_t*)malloc((size_t)length);
  input->size = (size_t)length;
  if (input->bytes == NULL || fread(input->bytes, 1, input->size, file) != input->size) {
    fclose(file);
    free(input->bytes);
    return 0;
  }
  fclose(file);
  return 1;
}

int main(int argc, char** argv) {
  const int per_file = argc >= 4 && strcmp(argv[1], "--per-file") == 0;
  const int first_input = per_file ? 3 : 2;
  if (argc <= first_input) {
    fprintf(stderr,
            "usage: libwebp_decode_bench [--per-file] <iterations> "
            "<files...>\n");
    return 1;
  }
  const int iterations = atoi(argv[per_file ? 2 : 1]);
  if (iterations <= 0) return 1;
  const int count = argc - first_input;
  Input* inputs = (Input*)calloc((size_t)count, sizeof(*inputs));
  if (inputs == NULL) return 1;
  for (int i = 0; i < count; ++i) {
    if (!ReadFile(argv[i + first_input], &inputs[i])) return 1;
  }

  uint64_t checksum = 0;
  size_t rgba_bytes = 0;
  if (per_file) {
    for (int i = 0; i < count; ++i) {
      const double item_started = NowMilliseconds();
      size_t item_rgba_bytes = 0;
      uint64_t item_checksum = 0;
      for (int iteration = 0; iteration < iterations; ++iteration) {
        int width = 0;
        int height = 0;
        uint8_t* rgba =
            WebPDecodeRGBA(inputs[i].bytes, inputs[i].size, &width, &height);
        if (rgba == NULL) {
          fprintf(stderr, "decode failed for input %d\n", i);
          return 1;
        }
        const size_t pixels = (size_t)width * (size_t)height;
        item_checksum += (uint64_t)width + (uint64_t)height + rgba[0];
        item_rgba_bytes += 4 * pixels;
        WebPFree(rgba);
      }
      printf("measurement\tlibwebp\t%s\t%d\t%.6f\t%zu\t%zu\t%llu\n",
             argv[i + first_input], iterations,
             NowMilliseconds() - item_started, inputs[i].size, item_rgba_bytes,
             (unsigned long long)item_checksum);
    }
    for (int i = 0; i < count; ++i) free(inputs[i].bytes);
    free(inputs);
    return 0;
  }

  const double started = NowMilliseconds();
  for (int iteration = 0; iteration < iterations; ++iteration) {
    for (int i = 0; i < count; ++i) {
      int width = 0;
      int height = 0;
      uint8_t* rgba = WebPDecodeRGBA(inputs[i].bytes, inputs[i].size, &width, &height);
      if (rgba == NULL) {
        fprintf(stderr, "decode failed for input %d\n", i);
        return 1;
      }
      const size_t pixels = (size_t)width * (size_t)height;
      checksum += (uint64_t)width + (uint64_t)height + rgba[0];
      rgba_bytes += 4 * pixels;
      WebPFree(rgba);
    }
  }
  const double elapsed = NowMilliseconds() - started;
  printf("decoder=libwebp files=%d decodes=%d rgba_bytes=%zu elapsed_ms=%.3f checksum=%llu\n",
         count, count * iterations, rgba_bytes, elapsed,
         (unsigned long long)checksum);
  for (int i = 0; i < count; ++i) free(inputs[i].bytes);
  free(inputs);
  return 0;
}
