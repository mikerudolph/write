#include <stddef.h>
#include <stdint.h>

typedef struct {
    uint32_t kind;
    uint32_t start;
    uint32_t end;
} WcSpan;

typedef struct {
    uint8_t *bytes;
    size_t length;
    float width;
    float height;
} WcImage;

char *wc_blocks_json(const uint8_t *pointer, size_t length);
void wc_free_string(char *pointer);
size_t wc_inline_spans(const uint8_t *pointer, size_t length, WcSpan *output, size_t capacity);
WcImage wc_render_block(const uint8_t *kind_pointer, size_t kind_length, const uint8_t *pointer, size_t length);
void wc_free_image(WcImage image);
