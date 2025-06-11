use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process;

#[derive(Debug, Deserialize)]
struct EventField {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
}

#[derive(Debug, Deserialize)]
struct EventInfo {
    id: u32,
    #[serde(default)]
    comment: String,
    #[serde(default)]
    payload: Vec<EventField>,
}

fn main() {
    // Get paths relative to the new crate location
    let script_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let parent_dir = script_dir.parent().unwrap().to_path_buf();

    // Define paths
    let config_path = script_dir.join("events.toml");
    let c_template_path = parent_dir.join("c/bootstrap.templ.h");
    let c_output_path = parent_dir.join("c/bootstrap.gen.h");
    let rust_template_path = parent_dir.join("rs/types.templ.rs");
    let rust_output_path = parent_dir.join("rs/types.gen.rs");

    // Type mapping from TOML to C for scalar types
    let c_type_map: BTreeMap<&str, &str> = [("u32", "u32"), ("u64", "u64")].into_iter().collect();

    // Type mapping from TOML to Rust for scalar types
    let rust_type_map: BTreeMap<&str, &str> =
        [("u32", "u32"), ("u64", "u64")].into_iter().collect();

    // Types that need buffer representation
    let buffer_types = ["char[]", "char[][]"];

    // Read and parse TOML file
    let config_content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading {}: {}", config_path.display(), err);
            process::exit(1);
        }
    };

    // Parse the TOML as a nested structure
    let parsed_config: BTreeMap<String, BTreeMap<String, EventInfo>> =
        match toml::from_str(&config_content) {
            Ok(parsed) => parsed,
            Err(err) => {
                eprintln!("Error parsing TOML: {}", err);
                process::exit(1);
            }
        };

    // Flatten the nested structure into event_type -> EventInfo
    let mut events = BTreeMap::new();
    for (category, tracepoints) in parsed_config {
        for (tracepoint, event_info) in tracepoints {
            let event_key = format!("{}.{}", category, tracepoint);
            events.insert(event_key, event_info);
        }
    }

    // Sort events by ID for consistent generation
    let mut sorted_events: Vec<_> = events.iter().collect();
    sorted_events.sort_by_key(|(_, info)| info.id);

    // Generate C code
    generate_c_code(
        &c_template_path,
        &c_output_path,
        &sorted_events,
        &c_type_map,
        &buffer_types,
    );

    // Generate Rust code
    generate_rust_code(
        &rust_template_path,
        &rust_output_path,
        &sorted_events,
        &rust_type_map,
        &buffer_types,
    );
}

fn generate_c_code(
    template_path: &PathBuf,
    output_path: &PathBuf,
    sorted_events: &[(&String, &EventInfo)],
    type_map: &BTreeMap<&str, &str>,
    buffer_types: &[&str],
) {
    // Read template file
    let template_content = match fs::read_to_string(template_path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading {}: {}", template_path.display(), err);
            process::exit(1);
        }
    };

    // Generate content for each template section
    let file_description_content = generate_c_file_description();
    let event_type_content = generate_c_event_type_enum(sorted_events);
    let payload_structs_content = generate_c_payload_structs(sorted_events, type_map, buffer_types);
    let event_type_to_string_content = generate_c_event_type_to_string(sorted_events);
    let payload_to_dynamic_allocation_roots_content =
        generate_c_payload_to_dynamic_allocation_roots(sorted_events, type_map, buffer_types);
    let payload_to_kv_array_content = generate_c_payload_to_kv_array(sorted_events);
    let get_payload_size_content = generate_c_get_payload_size(sorted_events);

    // Replace template sections
    let mut result = template_content;
    result = replace_template_section(&result, "file_description", &file_description_content);
    result = replace_template_section(&result, "event_type", &event_type_content);
    result = replace_template_section(&result, "payload_structs", &payload_structs_content);
    result = replace_template_section(
        &result,
        "event_type_to_string",
        &event_type_to_string_content,
    );
    result = replace_template_section(
        &result,
        "payload_to_dynamic_allocation_roots",
        &payload_to_dynamic_allocation_roots_content,
    );
    result = replace_template_section(&result, "payload_to_kv_array", &payload_to_kv_array_content);
    result = replace_template_section(&result, "get_payload_size", &get_payload_size_content);

    // Write the result file
    if let Err(err) = fs::write(output_path, result) {
        eprintln!("Error writing {}: {}", output_path.display(), err);
        process::exit(1);
    }

    println!("Generated {}", output_path.display());
}

fn generate_rust_code(
    template_path: &PathBuf,
    output_path: &PathBuf,
    sorted_events: &[(&String, &EventInfo)],
    type_map: &BTreeMap<&str, &str>,
    buffer_types: &[&str],
) {
    // Read template file
    let template_content = match fs::read_to_string(template_path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading {}: {}", template_path.display(), err);
            process::exit(1);
        }
    };

    // Generate content for each template section
    let file_description_content = generate_rust_file_description();
    let event_type_content = generate_rust_event_type_enum(sorted_events);
    let event_payload_content = generate_rust_event_payload_enum(sorted_events);
    let payload_structs_content =
        generate_rust_payload_structs(sorted_events, type_map, buffer_types);
    let payload_conversion_content =
        generate_rust_payload_conversion(sorted_events, type_map, buffer_types);
    let event_type_from_u32_content = generate_rust_event_type_from_u32(sorted_events);
    let event_type_to_string_content = generate_rust_event_type_to_string(sorted_events);

    // Replace template sections
    let mut result = template_content;
    result = replace_template_section(&result, "file_description", &file_description_content);
    result = replace_template_section(&result, "event_type", &event_type_content);
    result = replace_template_section(&result, "event_payload", &event_payload_content);
    result = replace_template_section(&result, "payload_structs", &payload_structs_content);
    result = replace_template_section(&result, "payload_conversion", &payload_conversion_content);
    result = replace_template_section(&result, "event_type_from_u32", &event_type_from_u32_content);
    result = replace_template_section(
        &result,
        "event_type_to_string",
        &event_type_to_string_content,
    );

    // Write the result file
    if let Err(err) = fs::write(output_path, result) {
        eprintln!("Error writing {}: {}", output_path.display(), err);
        process::exit(1);
    }

    println!("Generated {}", output_path.display());
}

fn replace_template_section(content: &str, section_name: &str, replacement: &str) -> String {
    let start_marker = format!("// templ_start:{}\n", section_name);
    let end_marker = format!("\n// templ_end:{}\n", section_name);

    if let Some(start_pos) = content.find(&start_marker) {
        if let Some(end_pos) = content.find(&end_marker) {
            let before = &content[..start_pos];
            let after = &content[end_pos + end_marker.len()..];
            return format!("{}{}\n{}", before, replacement, after);
        }
    }

    eprintln!("Warning: Could not find template section {}", section_name);
    content.to_string()
}

// C generation functions (existing ones)
fn generate_c_file_description() -> String {
    let lines = vec![
        "/* ========================================================================== */",
        "/*                           GENERATED FILE                                   */",
        "/* ========================================================================== */",
        "/*                                                                            */",
        "/*  This file is automatically generated from bootstrap.templ.h               */",
        "/*  DO NOT EDIT MANUALLY - changes will be overwritten                        */",
        "/*                                                                            */",
        "/*  Generator: ebpf/typegen/typegen.rs                                        */",
        "/*  Template:  ebpf/c/bootstrap.templ.h                                       */",
        "/*  Config:    ebpf/typegen/events.toml                                       */",
        "/*                                                                            */",
        "/*  To regenerate: `cd tracer-client/src/ebpf/c && make` (fast)               */",
        "/*  Alternative:   `cd tracer-client && cargo build` (slower)                 */",
        "/*                                                                            */",
        "/* ========================================================================== */",
    ];
    lines.join("\n")
}

fn generate_c_event_type_enum(sorted_events: &[(&String, &EventInfo)]) -> String {
    let header = vec!["enum event_type", "{"].join("\n");

    let mut enum_parts = Vec::new();
    for (category_tp, info) in sorted_events {
        let parts: Vec<&str> = category_tp.split('.').collect();
        if parts.len() != 2 {
            eprintln!("Invalid category.tracepoint format: {}", category_tp);
            process::exit(1);
        }
        let (category, tracepoint) = (parts[0], parts[1]);
        let enum_name = format!("event_type_{}_{}", category, tracepoint);
        enum_parts.push(format!("  {} = {},", enum_name, info.id));
    }

    let footer = vec!["};"].join("\n");

    format!("{}\n{}\n{}", header, enum_parts.join("\n"), footer)
}

fn generate_c_payload_structs(
    sorted_events: &[(&String, &EventInfo)],
    type_map: &BTreeMap<&str, &str>,
    buffer_types: &[&str],
) -> String {
    let mut lines = Vec::new();

    for (category_tp, info) in sorted_events {
        let parts: Vec<&str> = category_tp.split('.').collect();
        let (category, tracepoint) = (parts[0], parts[1]);

        if !info.comment.is_empty() {
            lines.push(format!("// {}", info.comment));
        }

        for &env in &["user", "kernel"] {
            let struct_name = format!("payload_{}_{}_{}", env, category, tracepoint);
            lines.push(format!("struct {}", struct_name));
            lines.push("{".into());

            if info.payload.is_empty() {
                lines.push("  char _unused; // Empty payload".into());
            } else {
                for field in &info.payload {
                    let decl = if buffer_types.contains(&field.field_type.as_str()) {
                        if env == "kernel" {
                            format!("  u64 {}; // Descriptor from buf_malloc_dyn\n  u32 _{}_unused; // Padding", field.name, field.name)
                        } else {
                            format!("  struct flex_buf {};", field.name)
                        }
                    } else {
                        let c_type = type_map.get(field.field_type.as_str()).unwrap_or(&"u64");
                        format!("  {} {};", c_type, field.name)
                    };
                    lines.push(decl);
                }
            }
            lines.push("} __attribute__((packed));".into());
        }
        lines.push("".to_string());
    }

    lines.join("\n")
}

fn generate_c_event_type_to_string(sorted_events: &[(&String, &EventInfo)]) -> String {
    let header = vec![
        "static inline const char* event_type_to_string(enum event_type t)",
        "{",
        "  switch (t)",
        "  {",
    ]
    .join("\n");

    let mut case_parts = Vec::new();
    for (category_tp, _) in sorted_events {
        let (category, tracepoint) = category_tp.split_once('.').unwrap();

        case_parts.push(
            vec![
                format!("  case event_type_{}_{}:", category, tracepoint),
                format!("    return \"{}/{}\";", category, tracepoint),
            ]
            .join("\n"),
        );
    }

    let footer = vec!["  default:", "    return \"unknown\";", "  }", "}"].join("\n");

    format!("{}\n{}\n{}", header, case_parts.join("\n"), footer)
}

fn generate_c_payload_to_dynamic_allocation_roots(
    sorted_events: &[(&String, &EventInfo)],
    _type_map: &BTreeMap<&str, &str>,
    buffer_types: &[&str],
) -> String {
    // ---------- prologue ----------
    let header = r#"static inline void
payload_to_dynamic_allocation_roots(enum event_type t,
                                    void *src_ptr,
                                    void *dst_ptr,
                                    struct dar_array *src_result,
                                    struct dar_array *dst_result)
{
  *src_result = (struct dar_array){0, NULL};
  *dst_result = (struct dar_array){0, NULL};
  switch (t)
  {"#;

    // ---------- one case per event that has dynamic fields ----------
    let mut case_parts = Vec::new();

    for (category_tp, info) in sorted_events {
        let (category, tracepoint) = category_tp.split_once('.').unwrap();

        let dynamic_fields: Vec<_> = info
            .payload
            .iter()
            .filter(|f| buffer_types.contains(&f.field_type.as_str()))
            .collect();

        if dynamic_fields.is_empty() {
            continue;
        }

        // begin case
        let mut lines = vec![
            format!("  case event_type_{}_{}:", category, tracepoint),
            "  {".into(),
            format!(
                "    struct payload_kernel_{}_{} *src = (struct payload_kernel_{}_{} *)src_ptr;",
                category, tracepoint, category, tracepoint
            ),
            format!(
                "    struct payload_user_{}_{} *dst = (struct payload_user_{}_{} *)dst_ptr;",
                category, tracepoint, category, tracepoint
            ),
            format!("    static u64 src_roots[{}];", dynamic_fields.len()),
            format!("    static u64 dst_roots[{}];", dynamic_fields.len()),
        ];

        for (i, field) in dynamic_fields.iter().enumerate() {
            lines.push(format!(
                "    src_roots[{i}] = (u64)&src->{name};",
                name = field.name
            ));
            lines.push(format!(
                "    dst_roots[{i}] = (u64)&dst->{name};",
                name = field.name
            ));
        }

        lines.push(format!(
            "    *src_result = (struct dar_array){{{}, src_roots}};",
            dynamic_fields.len()
        ));
        lines.push(format!(
            "    *dst_result = (struct dar_array){{{}, dst_roots}};",
            dynamic_fields.len()
        ));
        lines.push("    break;".into());
        lines.push("  }".into());

        case_parts.push(lines.join("\n"));
    }

    // ---------- epilogue ----------
    let footer = "  default:\n    break;\n  }\n}".to_string();

    format!("{header}\n{}\n{footer}", case_parts.join("\n"))
}

fn generate_c_payload_to_kv_array(sorted_events: &[(&String, &EventInfo)]) -> String {
    let header = vec![
        "static inline struct kv_array payload_to_kv_array(enum event_type t, void *ptr)",
        "{",
        "  struct kv_array result = {0, NULL};",
        "  switch (t)",
        "  {",
    ]
    .join("\n");

    let mut case_parts = Vec::new();
    for (category_tp, info) in sorted_events {
        let (category, tracepoint) = category_tp.split_once('.').unwrap();

        if !info.payload.is_empty() {
            let mut case_lines = vec![
                format!("  case event_type_{}_{}:", category, tracepoint),
                "  {".to_string(),
                format!(
                    "    struct payload_user_{}_{} *p = (struct payload_user_{}_{} *)ptr;",
                    category, tracepoint, category, tracepoint
                ),
                format!(
                    "    static struct kv_entry entries[{}];",
                    info.payload.len()
                ),
            ];

            for (i, field) in info.payload.iter().enumerate() {
                let type_str = field.field_type.as_str();

                case_lines.extend(vec![
                    format!("    strcpy(entries[{}].type, \"{}\");", i, type_str),
                    format!("    strcpy(entries[{}].key, \"{}\");", i, field.name),
                    format!("    entries[{}].value = &p->{};", i, field.name),
                ]);
            }

            case_lines.extend(vec![
                format!("    result.length = {};", info.payload.len()),
                "    result.data = entries;".to_string(),
                "    break;".to_string(),
                "  }".to_string(),
            ]);

            case_parts.push(case_lines.join("\n"));
        } else {
            // Handle empty payload events
            case_parts.push(
                vec![
                    format!("  case event_type_{}_{}:", category, tracepoint),
                    "    break;".to_string(),
                ]
                .join("\n"),
            );
        }
    }

    let footer = vec!["  default:", "    break;", "  }", "  return result;", "}"].join("\n");

    format!("{}\n{}\n{}", header, case_parts.join("\n"), footer)
}

fn generate_c_get_payload_size(sorted_events: &[(&String, &EventInfo)]) -> String {
    let header = vec![
        "static inline size_t get_payload_fixed_size(enum event_type t)",
        "{",
        "  switch (t)",
        "  {",
    ]
    .join("\n");

    let mut case_parts = Vec::new();
    for (category_tp, _) in sorted_events {
        let (category, tracepoint) = category_tp.split_once('.').unwrap();

        case_parts.push(
            vec![
                format!("  case event_type_{}_{}:", category, tracepoint),
                format!(
                    "    return sizeof(struct payload_kernel_{}_{});",
                    category, tracepoint
                ),
            ]
            .join("\n"),
        );
    }

    let footer = vec!["  default:", "    return 0;", "  }", "}"].join("\n");

    format!("{}\n{}\n{}", header, case_parts.join("\n"), footer)
}

// Rust generation functions (new)
fn generate_rust_file_description() -> String {
    let lines = vec![
        "/* ========================================================================== */",
        "/*                           GENERATED FILE                                   */",
        "/* ========================================================================== */",
        "/*                                                                            */",
        "/*  This file is automatically generated from types.templ.rs                 */",
        "/*  DO NOT EDIT MANUALLY - changes will be overwritten                        */",
        "/*                                                                            */",
        "/*  Generator: ebpf/typegen/typegen.rs                                        */",
        "/*  Template:  ebpf/rs/types.templ.rs                                         */",
        "/*  Config:    ebpf/typegen/events.toml                                       */",
        "/*                                                                            */",
        "/*  To regenerate: `cd tracer-client/src/ebpf/c && make` (fast)               */",
        "/*  Alternative:   `cd tracer-client && cargo build` (slower)                 */",
        "/*                                                                            */",
        "/* ========================================================================== */",
    ];
    lines.join("\n")
}

fn generate_rust_event_type_enum(sorted_events: &[(&String, &EventInfo)]) -> String {
    let header = vec![
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]",
        "#[repr(u32)]",
        "pub enum EventType {",
    ]
    .join("\n");

    let mut enum_parts = Vec::new();
    for (category_tp, info) in sorted_events {
        let parts: Vec<&str> = category_tp.split('.').collect();
        if parts.len() != 2 {
            eprintln!("Invalid category.tracepoint format: {}", category_tp);
            process::exit(1);
        }
        let (category, tracepoint) = (parts[0], parts[1]);

        // Convert to PascalCase
        let enum_name = to_pascal_case(&format!("{}_{}", category, tracepoint));
        enum_parts.push(format!("    {} = {},", enum_name, info.id));
    }

    // Add the Unknown variant at the end
    enum_parts.push("    // Add unknown variant for robustness".to_string());
    enum_parts.push("    Unknown(u32),".to_string());

    let footer = "}";

    format!("{}\n{}\n{}", header, enum_parts.join("\n"), footer)
}

fn generate_rust_event_payload_enum(sorted_events: &[(&String, &EventInfo)]) -> String {
    let header = vec![
        "#[derive(Debug, Clone, Serialize, Deserialize)]",
        "pub enum EventPayload {",
        "    Empty,",
    ]
    .join("\n");

    let mut enum_parts = Vec::new();
    for (category_tp, info) in sorted_events {
        if !info.payload.is_empty() {
            let parts: Vec<&str> = category_tp.split('.').collect();
            let (category, tracepoint) = (parts[0], parts[1]);

            // Convert to PascalCase
            let variant_name = to_pascal_case(&format!("{}_{}", category, tracepoint));
            let struct_name = format!("{}Payload", variant_name);
            enum_parts.push(format!("    {}({}),", variant_name, struct_name));
        }
    }

    let footer = "}";

    format!("{}\n{}\n{}", header, enum_parts.join("\n"), footer)
}

fn generate_rust_payload_structs(
    sorted_events: &[(&String, &EventInfo)],
    type_map: &BTreeMap<&str, &str>,
    buffer_types: &[&str],
) -> String {
    let mut lines = Vec::new();

    for (category_tp, info) in sorted_events {
        if info.payload.is_empty() {
            continue;
        }

        let parts: Vec<&str> = category_tp.split('.').collect();
        let (category, tracepoint) = (parts[0], parts[1]);

        if !info.comment.is_empty() {
            lines.push(format!("// {}", info.comment));
        }

        let variant_name = to_pascal_case(&format!("{}_{}", category, tracepoint));
        let struct_name = format!("{}Payload", variant_name);

        lines.push("#[derive(Debug, Clone, Serialize, Deserialize)]".to_string());
        lines.push(format!("pub struct {} {{", struct_name));

        for field in &info.payload {
            let rust_type = if buffer_types.contains(&field.field_type.as_str()) {
                match field.field_type.as_str() {
                    "char[]" => "String",
                    "char[][]" => "Vec<String>",
                    _ => "String",
                }
            } else {
                type_map.get(field.field_type.as_str()).unwrap_or(&"u64")
            };

            lines.push(format!("    pub {}: {},", field.name, rust_type));
        }

        lines.push("}".to_string());
        lines.push("".to_string());
    }

    lines.join("\n")
}

fn generate_rust_event_type_from_u32(sorted_events: &[(&String, &EventInfo)]) -> String {
    let header = vec![
        "impl From<u32> for EventType {",
        "    fn from(value: u32) -> Self {",
        "        match value {",
    ]
    .join("\n");

    let mut case_parts = Vec::new();
    for (category_tp, info) in sorted_events {
        let parts: Vec<&str> = category_tp.split('.').collect();
        let (category, tracepoint) = (parts[0], parts[1]);

        let enum_name = to_pascal_case(&format!("{}_{}", category, tracepoint));
        case_parts.push(format!(
            "            {} => EventType::{},",
            info.id, enum_name
        ));
    }

    let footer = vec![
        "            unknown => EventType::Unknown(unknown),",
        "        }",
        "    }",
        "}",
    ]
    .join("\n");

    format!("{}\n{}\n{}", header, case_parts.join("\n"), footer)
}

fn generate_rust_event_type_to_string(sorted_events: &[(&String, &EventInfo)]) -> String {
    let header = vec![
        "impl EventType {",
        "    pub fn as_str(&self) -> &'static str {",
        "        match self {",
    ]
    .join("\n");

    let mut case_parts = Vec::new();
    for (category_tp, _) in sorted_events {
        let (category, tracepoint) = category_tp.split_once('.').unwrap();

        let enum_name = to_pascal_case(&format!("{}_{}", category, tracepoint));
        case_parts.push(format!(
            "            EventType::{} => \"{}/{}\",",
            enum_name, category, tracepoint
        ));
    }

    // Add Unknown case
    case_parts.push("            EventType::Unknown(_) => \"unknown\",".to_string());

    let footer = vec!["        }", "    }", "}"].join("\n");

    format!("{}\n{}\n{}", header, case_parts.join("\n"), footer)
}

fn generate_rust_payload_conversion(
    sorted_events: &[(&String, &EventInfo)],
    type_map: &BTreeMap<&str, &str>,
    buffer_types: &[&str],
) -> String {
    let header = vec![
        "impl EventPayload {",
        "    pub unsafe fn from_c_payload(event_type: u32, payload_ptr: *mut c_void) -> Self {",
        "        match event_type {",
    ]
    .join("\n");

    let mut case_parts = Vec::new();
    for (category_tp, info) in sorted_events {
        if info.payload.is_empty() {
            continue;
        }

        let parts: Vec<&str> = category_tp.split('.').collect();
        let (category, tracepoint) = (parts[0], parts[1]);

        let variant_name = to_pascal_case(&format!("{}_{}", category, tracepoint));
        let struct_name = format!("{}Payload", variant_name);

        let mut case_lines = vec![
            format!("            {} => {{", info.id),
            format!(
                "                let c_payload = &*(payload_ptr as *const CPayload{});",
                variant_name
            ),
        ];

        // Generate field conversions
        let mut field_conversions = Vec::new();
        for field in &info.payload {
            let conversion = if buffer_types.contains(&field.field_type.as_str()) {
                match field.field_type.as_str() {
                    "char[]" => format!(
                        "                    {}: flex_buf_to_string(&c_payload.{}),",
                        field.name, field.name
                    ),
                    "char[][]" => format!(
                        "                    {}: flex_buf_to_string_array(&c_payload.{}),",
                        field.name, field.name
                    ),
                    _ => format!(
                        "                    {}: flex_buf_to_string(&c_payload.{}),",
                        field.name, field.name
                    ),
                }
            } else {
                format!(
                    "                    {}: c_payload.{},",
                    field.name, field.name
                )
            };
            field_conversions.push(conversion);
        }

        case_lines.push(format!(
            "                EventPayload::{}({} {{",
            variant_name, struct_name
        ));
        case_lines.extend(field_conversions);
        case_lines.push("                })".to_string());
        case_lines.push("            }".to_string());

        case_parts.push(case_lines.join("\n"));
    }

    let footer = vec![
        "            _ => EventPayload::Empty,",
        "        }",
        "    }",
        "}",
    ]
    .join("\n");

    // Also generate C struct definitions for each payload
    let mut c_struct_defs = Vec::new();
    for (category_tp, info) in sorted_events {
        if info.payload.is_empty() {
            continue;
        }

        let parts: Vec<&str> = category_tp.split('.').collect();
        let (category, tracepoint) = (parts[0], parts[1]);

        let variant_name = to_pascal_case(&format!("{}_{}", category, tracepoint));

        c_struct_defs.push(format!("// C struct for {}", category_tp));
        c_struct_defs.push("#[repr(C, packed)]".to_string());
        c_struct_defs.push(format!("struct CPayload{} {{", variant_name));

        for field in &info.payload {
            let field_type = if buffer_types.contains(&field.field_type.as_str()) {
                "FlexBuf"
            } else {
                type_map.get(field.field_type.as_str()).unwrap_or(&"u64")
            };
            c_struct_defs.push(format!("    {}: {},", field.name, field_type));
        }

        c_struct_defs.push("}".to_string());
        c_struct_defs.push("".to_string());
    }

    format!(
        "{}\n\n{}\n{}\n{}",
        c_struct_defs.join("\n"),
        header,
        case_parts.join("\n"),
        footer
    )
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}
