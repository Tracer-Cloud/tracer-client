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
constexpr size_t HEADER_BUFFER_SIZE = 512;        // 512 bytes for header
constexpr size_t PAYLOAD_BUFFER_SIZE = 64 * 1024; // 64KB for payload

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
      const struct flex_buf *fb = static_cast<const struct flex_buf *>(entry.value);
      if (fb && fb->byte_length > 0 && fb->data)
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
      const struct flex_buf *fb = static_cast<const struct flex_buf *>(entry.value);
      if (fb && fb->byte_length > 0 && fb->data)
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
    else
    {
      // Unknown type, print as generic pointer
      std::cout << "\"<" << entry.type << ">\"";
    }
  }
  std::cout << "}";
}

// ----------------------------------------------
// Callback function
// ----------------------------------------------
static void event_callback(header_ctx *header_ctx, payload_ctx *payload_ctx)
{
  // Validate inputs
  if (!header_ctx || !header_ctx->data || !payload_ctx)
  {
    std::cerr << "Error: Invalid context in event_callback" << std::endl;
    return;
  }

  // Process header data
  const struct event_header_user *header = header_ctx->data;

  // Print event as a single JSON object combining header and payload
  std::cout << "{";
  std::cout << "\"event_id\":" << header->event_id << ",";
  std::cout << "\"event_type\":\"" << event_type_to_string(header->event_type) << "\",";
  std::cout << "\"timestamp_ns\":" << header->timestamp_ns << ",";
  std::cout << "\"pid\":" << header->pid << ",";
  std::cout << "\"ppid\":" << header->ppid << ",";
  std::cout << "\"upid\":" << header->upid << ",";
  std::cout << "\"uppid\":" << header->uppid << ",";
  std::cout << "\"comm\":\"" << header->comm << "\"";

  // Process payload data if available
  if (payload_ctx->data != nullptr)
  {
    // Use payload_to_kv_array to get structured payload data
    struct kv_array kv_data = payload_to_kv_array(payload_ctx->event_type, payload_ctx->data);
    std::cout << ",\"payload\":{";
    print_kv_array_as_json(kv_data);
  }

  std::cout << "}" << std::endl;
}

/* Forward SIGINT/SIGTERM to the C API so bootstrap.c breaks its poll loop */
static void sig_handler(int) { tracer_ebpf_shutdown(); }

// ----------------------------------------------
// main()
// ----------------------------------------------
int main()
{
  // Allocate buffers for headers and payloads
  void *header_buf = std::malloc(HEADER_BUFFER_SIZE);
  if (!header_buf)
  {
    std::perror("malloc header_buf");
    return EXIT_FAILURE;
  }

  void *payload_buf = std::malloc(PAYLOAD_BUFFER_SIZE);
  if (!payload_buf)
  {
    std::perror("malloc payload_buf");
    std::free(header_buf);
    return EXIT_FAILURE;
  }

  // Initialize context structures
  header_ctx header_context = {
      .data = static_cast<struct event_header_user *>(header_buf)};

  payload_ctx payload_context = {
      .event_id = 0,                    // Will be set by the library
      .event_type = (enum event_type)0, // Will be set by the library
      .data = payload_buf,
      .size = PAYLOAD_BUFFER_SIZE};

  std::signal(SIGINT, sig_handler);
  std::signal(SIGTERM, sig_handler);

  int err = tracer_ebpf_initialize(&header_context, &payload_context, event_callback);

  std::free(header_buf);
  std::free(payload_buf);

  if (err)
  {
    std::fprintf(stderr, "tracer_ebpf_initialize() failed: %d\n", err);
    return EXIT_FAILURE;
  }

  return EXIT_SUCCESS;
}
