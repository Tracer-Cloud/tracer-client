#ifndef __BOOTSTRAP_API_H
#define __BOOTSTRAP_API_H

#include <stddef.h>

/**
 * Callback function type that will be invoked when events are ready.
 *
 * @param context User-provided context pointer
 * @param filled_bytes Number of bytes written to the buffer
 */
typedef void (*event_callback_t)(void *context, size_t filled_bytes);

/**
 * Initialize the kernel tracing and event processing.
 *
 * This function will start the BPF program, attach it to tracepoints,
 * and begin collecting events. When events are ready, they will be
 * written to the provided buffer and the callback will be invoked.
 *
 * @param buffer Pointer to a buffer where events will be written
 * @param byte_count Size of the buffer in bytes
 * @param callback Function to call when events are ready
 * @param callback_ctx Context pointer to pass to the callback
 * @return 0 on success, non-zero on error
 */
int initialize(void *buffer, size_t byte_count, event_callback_t callback, void *callback_ctx);

#endif /* __BOOTSTRAP_API_H */