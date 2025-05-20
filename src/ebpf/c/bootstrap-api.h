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
 * Structure for payload index entry.
 * Maps event IDs to their byte offsets within the payload data.
 */
struct payload_batch_index_entry
{
  u64 event_id;               // Unique identifier for the event
  enum event_type event_type; // Event type for payload parsing
  u32 offset;                 // Byte offset within payload_data
};

/**
 * Structure for payload batch header.
 * Contains metadata about a batch of event payloads.
 */
struct payload_batch_header
{
  u32 bytes_written;                               // Total bytes of payload data written
  u32 num_payloads;                                // Number of payloads in this batch
  struct payload_batch_index_entry *payload_index; // Array of index entries
  void *payload_data;                              // Pointer to the actual payload data
};

/**
 * Context structure for event payload batches.
 * Contains pointer to batch data and metadata for processing.
 */
typedef struct
{
  struct payload_batch_header *data; // Pointer to the batch header and data
  size_t size;                       // Space available in buffer
} payload_ctx;

/**
 * Callback invoked when an event header is received.
 * The callback must modify the header_ctx in-place to indicate where
 * the next header should be written.
 *
 * @param ctx Header context containing the current header data
 */
typedef void (*header_callback_t)(header_ctx *ctx);

/**
 * Callback function type that will be invoked when event payload batches are ready.
 * The callback must modify the payload_ctx in-place to indicate where
 * the next payload batch should be written.
 *
 * @param ctx Payload context containing the current batch data and size
 */
typedef void (*payload_callback_t)(payload_ctx *ctx);

#ifdef __cplusplus
extern "C"
{
#endif

  /**
   * Initialize the kernel tracing and event processing with 2-layer buffering.
   *
   * This function will start the BPF program, attach it to tracepoints,
   * and begin collecting events. Headers and payloads are processed separately
   * and delivered via different callbacks. The callbacks must modify the
   * provided contexts in-place to indicate where the next data should be written.
   *
   * @param header_ctx Context for header processing
   * @param payload_ctx Context for payload processing
   * @param header_callback Function to call when headers are ready
   * @param payload_callback Function to call when payload batches are ready
   * @return 0 on success, non-zero on error
   */
  int initialize(header_ctx *header_ctx, payload_ctx *payload_ctx,
                 header_callback_t header_callback, payload_callback_t payload_callback);

#ifdef __cplusplus
}
#endif

#endif /* __BOOTSTRAP_API_H */