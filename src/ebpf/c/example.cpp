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
constexpr size_t HEADER_BUFFER_SIZE = 512;         // 512 bytes for header
constexpr size_t PAYLOAD_BUFFER_SIZE = 256 * 1024; // 256KB for payload
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
  std::cout << "[DEBUG] print_kv_array_as_json: ENTRY, length=" << kv_array.length << std::endl;
  std::cout.flush();

  for (u32 i = 0; i < kv_array.length; i++)
  {

    if (i > 0)
      std::cout << ",";

    const struct kv_entry &entry = kv_array.data[i];

    if (!entry.key)
    {
      continue;
    }
    if (!entry.type)
    {
      continue;
    }
    if (!entry.value)
    {
      continue;
    }

    std::cout.flush();

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
        std::cout.flush();

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

  std::cout << "[DEBUG] print_kv_array_as_json: EXIT" << std::endl;
  std::cout.flush();
}

// ----------------------------------------------
// Callback function
// ----------------------------------------------
static void event_callback(header_ctx *header_ctx, payload_ctx *payload_ctx)
{
  std::cout << "[DEBUG] event_callback: ENTRY" << std::endl;
  std::cout.flush();

  // Validate inputs
  if (!header_ctx)
  {
    std::cout << "[DEBUG] event_callback: header_ctx is NULL!" << std::endl;
    std::cout.flush();
    return;
  }
  if (!header_ctx->data)
  {
    std::cout << "[DEBUG] event_callback: header_ctx->data is NULL!" << std::endl;
    std::cout.flush();
    return;
  }
  if (!payload_ctx)
  {
    std::cout << "[DEBUG] event_callback: payload_ctx is NULL!" << std::endl;
    std::cout.flush();
    return;
  }

  std::cout << "[DEBUG] event_callback: Processing header data" << std::endl;
  std::cout.flush();

  // Process header data
  const struct event_header_user *header = header_ctx->data;

  std::cout << "[DEBUG] event_callback: Header processed, event_id=" << header->event_id
            << ", event_type=" << (int)header->event_type << std::endl;
  std::cout.flush();

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

  std::cout << "[DEBUG] event_callback: Header JSON printed, checking payload" << std::endl;
  std::cout.flush();

  // Process payload data if available
  if (payload_ctx->data != nullptr)
  {
    std::cout << "[DEBUG] event_callback: Processing payload data" << std::endl;
    std::cout.flush();

    // Use payload_to_kv_array to get structured payload data
    struct kv_array kv_data = payload_to_kv_array(payload_ctx->event_type, payload_ctx->data);

    std::cout << "[DEBUG] event_callback: payload_to_kv_array returned, length=" << kv_data.length << std::endl;
    std::cout.flush();

    std::cout << ",\"payload\":";
    print_kv_array_as_json(kv_data);
  }
  else
  {
    std::cout << "[DEBUG] event_callback: No payload data (payload_ctx->data is null)" << std::endl;
    std::cout.flush();
  }

  std::cout << "}" << std::endl;
  std::cout.flush();

  std::cout << "[DEBUG] event_callback: EXIT" << std::endl;
  std::cout.flush();

  // The callback can modify both contexts to indicate where the next data should be written
  // For this simple example, we'll leave them as-is, so the next write overwrites the previous
}

static void sig_handler(int) { exiting = 1; }

// ----------------------------------------------
// main()
// ----------------------------------------------
int main()
{
  std::cout << "[DEBUG] main: ENTRY" << std::endl;
  std::cout.flush();

  // Allocate buffers for headers and payloads
  std::cout << "[DEBUG] main: Allocating header buffer (" << HEADER_BUFFER_SIZE << " bytes)" << std::endl;
  std::cout.flush();

  void *header_buf = std::malloc(HEADER_BUFFER_SIZE);
  if (!header_buf)
  {
    std::cout << "[DEBUG] main: header_buf allocation FAILED" << std::endl;
    std::cout.flush();
    std::perror("malloc header_buf");
    return EXIT_FAILURE;
  }
  std::cout << "[DEBUG] main: header_buf allocated at " << header_buf << std::endl;
  std::cout.flush();

  std::cout << "[DEBUG] main: Allocating payload buffer (" << PAYLOAD_BUFFER_SIZE << " bytes)" << std::endl;
  std::cout.flush();

  void *payload_buf = std::malloc(PAYLOAD_BUFFER_SIZE);
  if (!payload_buf)
  {
    std::cout << "[DEBUG] main: payload_buf allocation FAILED" << std::endl;
    std::cout.flush();
    std::perror("malloc payload_buf");
    std::free(header_buf);
    return EXIT_FAILURE;
  }
  std::cout << "[DEBUG] main: payload_buf allocated at " << payload_buf << std::endl;
  std::cout.flush();

  // Initialize context structures
  std::cout << "[DEBUG] main: Initializing context structures" << std::endl;
  std::cout.flush();

  header_ctx header_context = {
      .data = static_cast<struct event_header_user *>(header_buf)};

  payload_ctx payload_context = {
      .event_id = 0,                    // Will be set by the library
      .event_type = (enum event_type)0, // Will be set by the library
      .data = payload_buf,
      .size = PAYLOAD_BUFFER_SIZE};

  std::cout << "[DEBUG] main: Context structures initialized" << std::endl;
  std::cout << "[DEBUG] main: header_context.data = " << header_context.data << std::endl;
  std::cout << "[DEBUG] main: payload_context.data = " << payload_context.data << std::endl;
  std::cout << "[DEBUG] main: payload_context.size = " << payload_context.size << std::endl;
  std::cout.flush();

  std::signal(SIGINT, sig_handler);
  std::signal(SIGTERM, sig_handler);

  std::cout << "Starting eBPF event logger – press Ctrl+C to stop...\n";
  std::cout.flush();

  std::cout << "[DEBUG] main: Calling initialize() with callback function" << std::endl;
  std::cout.flush();

  int err = initialize(&header_context, &payload_context, event_callback);

  std::cout << "[DEBUG] main: initialize() returned with err=" << err << std::endl;
  std::cout.flush();

  std::cout << "[DEBUG] main: Freeing buffers" << std::endl;
  std::cout.flush();

  std::free(header_buf);
  std::free(payload_buf);

  std::cout << "[DEBUG] main: Buffers freed" << std::endl;
  std::cout.flush();

  if (err)
  {
    std::fprintf(stderr, "initialize() failed: %d\n", err);
    return EXIT_FAILURE;
  }

  std::cout << "[DEBUG] main: EXIT (success)" << std::endl;
  std::cout.flush();

  return EXIT_SUCCESS;
}
