#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "webp/decode.h"

static uint8_t *read_file(const char *path, size_t *size) {
  FILE *file = fopen(path, "rb");
  uint8_t *data;
  long length;
  if (file == NULL || fseek(file, 0, SEEK_END) != 0 ||
      (length = ftell(file)) < 0 || fseek(file, 0, SEEK_SET) != 0) {
    if (file != NULL) fclose(file);
    return NULL;
  }
  data = (uint8_t *)malloc((size_t)length);
  if (data == NULL || fread(data, 1, (size_t)length, file) != (size_t)length) {
    free(data);
    fclose(file);
    return NULL;
  }
  fclose(file);
  *size = (size_t)length;
  return data;
}

static const char *base_name(const char *path) {
  const char *slash = strrchr(path, '/');
  return slash == NULL ? path : slash + 1;
}

int main(int argc, char **argv) {
  const char *expected_root;
  int matched = 0;
  int failed = 0;
  if (argc < 3) {
    fprintf(stderr,
            "usage: libwebp_vp8l_product_compare <expected-dir> <streams...>\n");
    return 2;
  }
  expected_root = argv[1];
  for (int i = 2; i < argc; ++i) {
    size_t encoded_size = 0, expected_size = 0;
    uint8_t *encoded = read_file(argv[i], &encoded_size);
    uint8_t *expected;
    uint8_t *decoded;
    int width = 0, height = 0;
    char expected_path[4096];
    char id[512];
    const char *name = base_name(argv[i]);
    size_t name_len = strlen(name);
    if (name_len < 6 || strcmp(name + name_len - 5, ".webp") != 0 ||
        name_len - 5 >= sizeof(id)) {
      fprintf(stderr, "%s: invalid name\n", argv[i]);
      return 1;
    }
    memcpy(id, name, name_len - 5);
    id[name_len - 5] = '\0';
    if (snprintf(expected_path, sizeof(expected_path), "%s/%s.rgba",
                 expected_root, id) >= (int)sizeof(expected_path)) {
      fprintf(stderr, "%s: expected path too long\n", argv[i]);
      return 1;
    }
    expected = read_file(expected_path, &expected_size);
    if (encoded == NULL || expected == NULL ||
        !WebPGetInfo(encoded, encoded_size, &width, &height)) {
      fprintf(stderr, "%s: read/info failure\n", argv[i]);
      free(encoded);
      free(expected);
      ++failed;
      continue;
    }
    if ((size_t)width * (size_t)height * 4 != expected_size) {
      fprintf(stderr, "%s: expected size mismatch\n", argv[i]);
      free(encoded);
      free(expected);
      ++failed;
      continue;
    }
    decoded = WebPDecodeRGBA(encoded, encoded_size, &width, &height);
    if (decoded == NULL || memcmp(decoded, expected, expected_size) != 0) {
      fprintf(stderr,
              "%s: WebPDecodeRGBA returned NULL or decoded bytes differ\n",
              argv[i]);
      WebPFree(decoded);
      free(encoded);
      free(expected);
      ++failed;
      continue;
    }
    printf("oracle\t%s\t%d\t%d\t%zu\n", id, width, height, expected_size);
    ++matched;
    WebPFree(decoded);
    free(encoded);
    free(expected);
  }
  printf("oracle_summary\tmatched=%d\tfailed=%d\n", matched, failed);
  return failed == 0 ? 0 : 1;
}
