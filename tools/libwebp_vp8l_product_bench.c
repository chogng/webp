#include <dirent.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "webp/decode.h"

typedef struct {
  char *id;
  uint8_t *encoded;
  size_t encoded_size;
  uint8_t *expected;
  size_t expected_size;
} Input;

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

static uint64_t now_ns(void) {
  struct timespec value;
  if (clock_gettime(CLOCK_MONOTONIC, &value) != 0) return 0;
  return (uint64_t)value.tv_sec * 1000000000u + (uint64_t)value.tv_nsec;
}

static uint64_t fnv1a(const uint8_t *bytes, size_t size) {
  uint64_t hash = UINT64_C(0xcbf29ce484222325);
  for (size_t i = 0; i < size; ++i) {
    hash = (hash ^ bytes[i]) * UINT64_C(0x100000001b3);
  }
  return hash;
}

static int compare_names(const void *left, const void *right) {
  const Input *a = (const Input *)left;
  const Input *b = (const Input *)right;
  return strcmp(a->id, b->id);
}

static int has_webp_suffix(const char *name) {
  const size_t length = strlen(name);
  return length > 5 && strcmp(name + length - 5, ".webp") == 0;
}

static void free_inputs(Input *inputs, size_t count) {
  for (size_t i = 0; i < count; ++i) {
    free(inputs[i].id);
    free(inputs[i].encoded);
    free(inputs[i].expected);
  }
  free(inputs);
}

static int load_inputs(const char *stream_root, const char *expected_root,
                       Input **result, size_t *result_count) {
  DIR *directory = opendir(stream_root);
  struct dirent *entry;
  Input *inputs = NULL;
  size_t count = 0;
  size_t capacity = 0;
  if (directory == NULL) return 0;
  while ((entry = readdir(directory)) != NULL) {
    char stream_path[4096];
    char expected_path[4096];
    size_t name_length;
    Input input;
    if (!has_webp_suffix(entry->d_name)) continue;
    memset(&input, 0, sizeof(input));
    name_length = strlen(entry->d_name) - 5;
    input.id = (char *)malloc(name_length + 1);
    if (input.id == NULL) goto fail;
    memcpy(input.id, entry->d_name, name_length);
    input.id[name_length] = '\0';
    if (snprintf(stream_path, sizeof(stream_path), "%s/%s", stream_root,
                 entry->d_name) >= (int)sizeof(stream_path) ||
        snprintf(expected_path, sizeof(expected_path), "%s/%s.rgba",
                 expected_root, input.id) >= (int)sizeof(expected_path)) {
      free(input.id);
      goto fail;
    }
    input.encoded = read_file(stream_path, &input.encoded_size);
    input.expected = read_file(expected_path, &input.expected_size);
    if (input.encoded == NULL || input.expected == NULL) {
      free(input.id);
      free(input.encoded);
      free(input.expected);
      goto fail;
    }
    if (count == capacity) {
      const size_t next = capacity == 0 ? 128 : capacity * 2;
      Input *grown = (Input *)realloc(inputs, next * sizeof(*inputs));
      if (grown == NULL) {
        free(input.id);
        free(input.encoded);
        free(input.expected);
        goto fail;
      }
      inputs = grown;
      capacity = next;
    }
    inputs[count++] = input;
  }
  closedir(directory);
  if (count == 0) goto fail_after_close;
  qsort(inputs, count, sizeof(*inputs), compare_names);
  *result = inputs;
  *result_count = count;
  return 1;

fail:
  closedir(directory);
fail_after_close:
  free_inputs(inputs, count);
  return 0;
}

int main(int argc, char **argv) {
  Input *inputs;
  size_t count;
  size_t rgba_bytes = 0;
  size_t input_bytes = 0;
  uint64_t aggregate_hash = 0;
  uint64_t aggregate_started;
  if (argc != 5) {
    fprintf(stderr,
            "usage: libwebp_vp8l_product_bench <round> <layout> "
            "<expected-dir> <stream-dir>\n");
    return 2;
  }
  if (!load_inputs(argv[4], argv[3], &inputs, &count)) {
    fprintf(stderr, "failed to preload benchmark inputs\n");
    return 1;
  }
  aggregate_started = now_ns();
  for (size_t i = 0; i < count; ++i) {
    int width = 0;
    int height = 0;
    uint8_t *decoded;
    uint64_t hash;
    uint64_t started = now_ns();
    uint64_t elapsed;
    decoded = WebPDecodeRGBA(inputs[i].encoded, inputs[i].encoded_size, &width,
                             &height);
    if (decoded == NULL || (size_t)width * (size_t)height * 4 !=
                               inputs[i].expected_size ||
        memcmp(decoded, inputs[i].expected, inputs[i].expected_size) != 0) {
      fprintf(stderr, "%s: decoded RGBA mismatch\n", inputs[i].id);
      WebPFree(decoded);
      free_inputs(inputs, count);
      return 1;
    }
    hash = fnv1a(decoded, inputs[i].expected_size);
    elapsed = now_ns() - started;
    rgba_bytes += inputs[i].expected_size;
    input_bytes += inputs[i].encoded_size;
    aggregate_hash ^= hash << (i % 17);
    printf("measurement\tdecode\t%s\t%s\t%s\t%llu\t%zu\t%zu\t%016llx\n",
           argv[1], argv[2], inputs[i].id, (unsigned long long)elapsed,
           inputs[i].expected_size, inputs[i].encoded_size,
           (unsigned long long)hash);
    WebPFree(decoded);
  }
  printf("aggregate\tdecode\t%s\t%s\t%zu\t%llu\t%zu\t%zu\t%016llx\n",
         argv[1], argv[2], count,
         (unsigned long long)(now_ns() - aggregate_started), rgba_bytes,
         input_bytes, (unsigned long long)aggregate_hash);
  free_inputs(inputs, count);
  return 0;
}
