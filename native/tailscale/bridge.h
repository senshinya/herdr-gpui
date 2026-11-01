#ifndef HERDR_TAILSCALE_BRIDGE_H
#define HERDR_TAILSCALE_BRIDGE_H
#include <stdint.h>
#include <stddef.h>
/* Buffers are borrowed only for the call. op: 0=read, 1=write, 2=delete.
 * Return 0=success, 1=missing, negative=storage failure. */
typedef int32_t (*herdr_store_fn)(uintptr_t, uint32_t, const uint8_t*, size_t, uint8_t*, size_t, size_t*);
static inline int32_t herdr_store_call(herdr_store_fn fn, uintptr_t context, uint32_t op,
    const uint8_t *key, size_t key_len, uint8_t *data, size_t capacity, size_t *length) {
    return fn(context, op, key, key_len, data, capacity, length);
}
#endif
