#ifndef __BOOTSTRAP_API_H
#define __BOOTSTRAP_API_H

#include <stddef.h>
#include "bootstrap.gen.h"

/**
 * Context structure for event headers.
 * Contains pointer to where the current header is located.
 */
typedef struct
{
  struct event_header_user *data;
} header_ctx;

/**
 * Context structure for a single event payload.
 * Contains pointer to payload data and metadata for processing.
 */
typedef struct
{
  u64 event_id;               // Event ID this payload belongs to
  enum event_type event_type; // Event type for payload parsing
  void *data;                 // Pointer to the actual payload data
  size_t size;                // Space available in buffer
} payload_ctx;

/**
 * Callback function type that will be invoked when event data is ready.
 * The callback receives both header and payload contexts and must modify them
 * in-place to indicate where the next data should be written.
 *
 * @param header_ctx Header context containing the current header data
 * @param payload_ctx Payload context containing the current payload data and size
 */
typedef void (*event_callback_t)(header_ctx *header_ctx, payload_ctx *payload_ctx);

#ifdef __cplusplus
extern "C"
{
#endif

  /**
   * Initialize the kernel tracing and event processing.
   *
   * This function will start the BPF program, attach it to tracepoints,
   * and begin collecting events. Headers and payloads are processed separately
   * but delivered via a single callback. The callback must modify the
   * provided contexts in-place to indicate where the next data should be written.
   *
   * @param header_ctx Context for header processing
   * @param payload_ctx Context for payload processing
   * @param callback Function to call when event data is ready
   * @return 0 on success, non-zero on error
   */
  int initialize(header_ctx *header_ctx, payload_ctx *payload_ctx,
                 event_callback_t callback);

#ifdef __cplusplus
}
#endif

#endif /* __BOOTSTRAP_API_H */