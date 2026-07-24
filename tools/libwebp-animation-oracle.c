#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include <webp/demux.h>

static int WriteU32LE(FILE* output, uint32_t value) {
  const uint8_t bytes[4] = {
      (uint8_t)value,
      (uint8_t)(value >> 8),
      (uint8_t)(value >> 16),
      (uint8_t)(value >> 24),
  };
  return fwrite(bytes, sizeof(bytes), 1, output) == 1;
}

int main(int argc, char** argv) {
  FILE* input = NULL;
  FILE* output = NULL;
  long input_size;
  uint8_t* input_bytes;
  WebPData data;
  WebPAnimDecoderOptions options;
  WebPAnimDecoder* decoder;
  WebPAnimInfo info;
  size_t frame_bytes;
  int ok = 0;

  if (argc != 3) return 2;
  input = fopen(argv[1], "rb");
  output = fopen(argv[2], "wb");
  if (input == NULL || output == NULL) goto End;
  if (fseek(input, 0, SEEK_END) != 0) goto End;
  input_size = ftell(input);
  if (input_size < 0 || fseek(input, 0, SEEK_SET) != 0) goto End;
  input_bytes = malloc((size_t)input_size);
  if (input_bytes == NULL && input_size != 0) goto End;
  if (fread(input_bytes, 1, (size_t)input_size, input) != (size_t)input_size) {
    free(input_bytes);
    goto End;
  }
  data.bytes = input_bytes;
  data.size = (size_t)input_size;
  if (!WebPAnimDecoderOptionsInit(&options)) {
    free(input_bytes);
    goto End;
  }
  options.color_mode = MODE_RGBA;
  decoder = WebPAnimDecoderNew(&data, &options);
  if (decoder == NULL || !WebPAnimDecoderGetInfo(decoder, &info)) {
    WebPAnimDecoderDelete(decoder);
    free(input_bytes);
    goto End;
  }
  frame_bytes = (size_t)info.canvas_width * info.canvas_height * 4;
  while (WebPAnimDecoderHasMoreFrames(decoder)) {
    uint8_t* frame;
    int timestamp;
    if (!WebPAnimDecoderGetNext(decoder, &frame, &timestamp) ||
        !WriteU32LE(output, (uint32_t)timestamp) ||
        fwrite(frame, frame_bytes, 1, output) != 1) {
      WebPAnimDecoderDelete(decoder);
      free(input_bytes);
      goto End;
    }
  }
  WebPAnimDecoderDelete(decoder);
  free(input_bytes);
  ok = 1;

End:
  if (input != NULL) fclose(input);
  if (output != NULL) fclose(output);
  return ok ? 0 : 1;
}
