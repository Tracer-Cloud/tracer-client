#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <iostream>
#include <vector>
#include <cstring> // for strcmp and memcpy

extern "C"
{
#include "bootstrap.gen.h"
#include "bootstrap-api.h"
}

// ----------------------------------------------
// Constants & Globals
// ----------------------------------------------
constexpr size_t HEADER_BUFFER_SIZE = 512 * 1024;   // 512 KB for headers
constexpr size_t PAYLOAD_BUFFER_SIZE = 1024 * 1024; // 1024 KB for payloads
static volatile sig_atomic_t exiting = 0;

// ----------------------------------------------
// Helper utilities
// ----------------------------------------------

// Helper function to print escaped characters for JSON strings
static void print_escaped_char(char c)
{
  if (c == '"')
    std::cout << "\\\"";
  else if (c == '\\')
    std::cout << "\\\\";
  else if (c == '\n')
    std::cout << "\\n";
  else if (c == '\r')
    std::cout << "\\r";
  else if (c == '\t')
    std::cout << "\\t";
  else
    std::cout << c;
}

// Helper function to print kv_array as JSON
static void print_kv_array_as_json(const struct kv_array &kv_array)
{
  std::cout << "{";
  for (u32 i = 0; i < kv_array.length; i++)
  {
    if (i > 0)
      std::cout << ",";

    const struct kv_entry &entry = kv_array.data[i];
    std::cout << "\"" << entry.key << "\":";

    // Print value based on type
    if (strcmp(entry.type, "u32") == 0)
    {
      std::cout << *static_cast<u32 *>(entry.value);
    }
    else if (strcmp(entry.type, "u64") == 0)
    {
      std::cout << *static_cast<u64 *>(entry.value);
    }
    else if (strcmp(entry.type, "char") == 0)
    {
      std::cout << "\"" << static_cast<char *>(entry.value) << "\"";
    }
    else if (strcmp(entry.type, "char[]") == 0)
    {
      // Handle flex_buf for strings
      struct flex_buf **flex_buf_ptr = static_cast<struct flex_buf **>(entry.value);
      struct flex_buf *fb = *flex_buf_ptr;
      if (fb && fb->byte_length > 0)
      {
        std::cout << "\"";
        // Print the string, ensuring null termination is handled
        for (u32 j = 0; j < fb->byte_length && fb->data[j] != '\0'; j++)
        {
          print_escaped_char(fb->data[j]);
        }
        std::cout << "\"";
      }
      else
      {
        std::cout << "null";
      }
    }
    else if (strcmp(entry.type, "char[][]") == 0)
    {
      // Handle flex_buf for string arrays (null-separated strings)
      struct flex_buf **flex_buf_ptr = static_cast<struct flex_buf **>(entry.value);
      struct flex_buf *fb = *flex_buf_ptr;
      if (fb && fb->byte_length > 0)
      {
        std::cout << "[";
        bool first_string = true;
        u32 start = 0;

        for (u32 j = 0; j <= fb->byte_length; j++)
        {
          // Check for null terminator or end of buffer
          if (j == fb->byte_length || fb->data[j] == '\0')
          {
            if (j > start) // Non-empty string
            {
              if (!first_string)
                std::cout << ",";
              std::cout << "\"";

              // Print the string with proper escaping
              for (u32 k = start; k < j; k++)
              {
                print_escaped_char(fb->data[k]);
              }

              std::cout << "\"";
              first_string = false;
            }
            start = j + 1;
          }
        }
        std::cout << "]";
      }
      else
      {
        std::cout << "[]";
      }
    }
    else if (strcmp(entry.type, "flex_buf") == 0)
    {
      // For now, just indicate it's a flexible buffer (as requested, don't print content)
      std::cout << "\"<flex_buf>\"";
    }
    else
    {
      // Unknown type, print as generic pointer
      std::cout << "\"<" << entry.type << ">\"";
    }
  }
  std::cout << "}";
}

// ----------------------------------------------
// Callback functions
// ----------------------------------------------
static void header_callback(header_ctx *ctx)
{
  // The header data is already populated in ctx->data by the library
  const struct event_header_user *header = ctx->data;

  // Print headers immediately since payloads are handled separately
  std::cout << "{";
  std::cout << "\"event_id\":" << header->event_id << ",";
  std::cout << "\"event_type\":\"" << event_type_to_string(header->event_type) << "\",";
  std::cout << "\"timestamp_ns\":" << header->timestamp_ns << ",";
  std::cout << "\"pid\":" << header->pid << ",";
  std::cout << "\"ppid\":" << header->ppid << ",";
  std::cout << "\"upid\":" << header->upid << ",";
  std::cout << "\"uppid\":" << header->uppid << ",";
  std::cout << "\"comm\":\"" << header->comm << "\"";
  std::cout << "}" << std::endl;

  // The callback can modify ctx->data to indicate where the next header should be written
  // For this simple example, we'll leave it as-is, so the next write overwrites the previous
}

static void payload_callback(payload_ctx *ctx)
{
  // The payload data is already populated in ctx->data by the library
  struct payload_batch_header *batch = ctx->data;

  if (!batch || batch->num_payloads == 0)
  {
    std::cout << "{\"payload_batch\":\"empty\"}" << std::endl;
    return;
  }

  std::cout << "{\"payload_batch\":{";
  std::cout << "\"num_payloads\":" << batch->num_payloads << ",";
  std::cout << "\"bytes_written\":" << batch->bytes_written << ",";
  std::cout << "\"payloads\":[";

  // Process each payload in the batch
  for (u32 i = 0; i < batch->num_payloads; i++)
  {
    if (i > 0)
      std::cout << ",";

    const struct payload_batch_index_entry &index_entry = batch->payload_index[i];

    // Calculate payload address
    char *payload_data = static_cast<char *>(batch->payload_data) + index_entry.offset;

    std::cout << "{";
    std::cout << "\"event_id\":" << index_entry.event_id << ",";
    std::cout << "\"event_type\":\"" << event_type_to_string(index_entry.event_type) << "\",";
    std::cout << "\"offset\":" << index_entry.offset << ",";

    // Use payload_to_kv_array to get structured payload data
    struct kv_array kv_data = payload_to_kv_array(index_entry.event_type, payload_data);
    std::cout << "\"payload\":";
    print_kv_array_as_json(kv_data);

    std::cout << "}";
  }

  std::cout << "]}}";
  std::cout << std::endl;

  // Reset for next batch - point to a fresh area of the buffer
  // Calculate total space used for this batch
  size_t total_batch_size = sizeof(struct payload_batch_header) +
                            (batch->num_payloads * sizeof(struct payload_batch_index_entry)) +
                            batch->bytes_written;

  // Move to next available space in buffer
  char *next_buffer = reinterpret_cast<char *>(ctx->data) + total_batch_size;
  size_t remaining_size = ctx->size - total_batch_size;

  ctx->data = reinterpret_cast<struct payload_batch_header *>(next_buffer);
  ctx->size = remaining_size;
}

static void sig_handler(int) { exiting = 1; }

// ----------------------------------------------
// main()
// ----------------------------------------------
int main()
{
  // Allocate buffers for headers and payloads
  void *header_buf = std::malloc(HEADER_BUFFER_SIZE);
  void *payload_buf = std::malloc(PAYLOAD_BUFFER_SIZE);

  if (!header_buf || !payload_buf)
  {
    std::perror("malloc");
    return EXIT_FAILURE;
  }

  // Initialize context structures
  header_ctx header_context = {
      .data = static_cast<struct event_header_user *>(header_buf)};

  payload_ctx payload_context = {
      .data = static_cast<struct payload_batch_header *>(payload_buf),
      .size = PAYLOAD_BUFFER_SIZE};

  std::signal(SIGINT, sig_handler);
  std::signal(SIGTERM, sig_handler);

  std::cout << "Starting eBPF event logger – press Ctrl+C to stop...\n";

  int err = initialize(&header_context, &payload_context,
                       header_callback, payload_callback);

  std::free(header_buf);
  std::free(payload_buf);

  if (err)
  {
    std::fprintf(stderr, "initialize() failed: %d\n", err);
    return EXIT_FAILURE;
  }

  return EXIT_SUCCESS;
}
