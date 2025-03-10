use crate::alias_map::AliasMap;
use crate::builder::BindgenOptions;
use crate::doc_comment::escape_doc_comment;
use crate::type_meta::ExportSymbolNaming::{ExportName, NoMangle};
use crate::type_meta::*;
use crate::util::*;

pub fn emit_rust_method(list: &Vec<ExternMethod>, options: &BindgenOptions) -> String {
    // configure
    let method_type_path = options.rust_method_type_path.as_str();
    let method_type_path2 = match options.rust_method_type_path.as_str() {
        "" => "".to_string(),
        x => x.to_string() + "::",
    };
    let method_prefix = &options.rust_method_prefix;
    let file_header = &options.rust_file_header;

    let mut methods_string = String::new();

    for item in list {
        let method_name = item.method_name.as_str();
        let parameters = item
            .parameters
            .iter()
            .map(|p| {
                format!(
                    "    {}: {}",
                    p.name,
                    p.rust_type.to_rust_string(method_type_path)
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");

        let return_line = match &item.return_type {
            None => "".to_string(),
            Some(v) => format!(" -> {}", v.to_rust_string(method_type_path)),
        };

        let parameter_only_names = item
            .parameters
            .iter()
            .map(|p| format!("        {}", p.name))
            .collect::<Vec<_>>()
            .join(",\n");

        let template = format!(
            "
#[no_mangle]
pub unsafe extern \"C\" fn {method_prefix}{method_name}(
{parameters}
){return_line}
{{
    {method_type_path2}{method_name}(
{parameter_only_names}
    )
}}
"
        );

        methods_string.push_str(template.as_str());
    }

    let result = format!(
        "/* automatically generated by csbindgen */

#[allow(unused)]
use ::std::os::raw::*;

{file_header}

{methods_string}
    "
    );

    result
}

pub fn emit_csharp(
    methods: &Vec<ExternMethod>,
    aliases: &AliasMap,
    structs: &Vec<RustStruct>,
    enums: &Vec<RustEnum>,
    consts: &Vec<RustConst>,
    options: &BindgenOptions,
) -> String {
    // configure
    let namespace = &options.csharp_namespace;
    let class_name = &options.csharp_class_name;
    let method_prefix = &options.csharp_method_prefix;
    let accessibility = &options.csharp_class_accessibility;

    let mut dll_name = match options.csharp_if_symbol.as_str() {
        "" => format!(
            "        const string __DllName = \"{}\";",
            options.csharp_dll_name
        ),
        _ => {
            format!(
                "#if {0}
        const string __DllName = \"{1}\";
#else
        const string __DllName = \"{2}\";
#endif
        ",
                options.csharp_if_symbol, options.csharp_if_dll_name, options.csharp_dll_name
            )
        }
    };
    if options.csharp_disable_emit_dll_name {
        dll_name = "".to_string();
    }

    let mut method_list_string = String::new();
    for item in methods {
        let mut method_name = &item.method_name;
        let method_name_temp: String;
        if method_prefix.is_empty() {
            method_name_temp = escape_csharp_name(method_name);
            method_name = &method_name_temp;
        }

        if let Some(x) = &item.return_type {
            if let Some(delegate_method) = build_method_delegate_if_required(
                x,
                options,
                aliases,
                method_name,
                &"return".to_string(),
            ) {
                method_list_string.push_str(
                    "        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]\n"
                        .to_string()
                        .as_str(),
                );
                method_list_string
                    .push_str(format!("        {accessibility} {delegate_method};\n\n").as_str());
            }
        }

        for p in item.parameters.iter() {
            if let Some(delegate_method) = build_method_delegate_if_required(
                &p.rust_type,
                options,
                aliases,
                method_name,
                &p.name,
            ) {
                method_list_string.push_str(
                    "        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]\n"
                        .to_string()
                        .as_str(),
                );
                method_list_string
                    .push_str(format!("        {accessibility} {delegate_method};\n\n").as_str());
            }
        }

        let entry_point = match &item.export_naming {
            NoMangle => &item.method_name,
            ExportName(export_name) => export_name,
        };

        let entry_point = match options.csharp_entry_point_prefix.as_str() {
            "" => format!("{method_prefix}{entry_point}"),
            x => format!("{x}{entry_point}"),
        };
        let return_type = match &item.return_type {
            Some(x) => {
                x.to_csharp_string(options, aliases, false, method_name, &"return".to_string())
            }
            None => "void".to_string(),
        };

        let parameters = item
            .parameters
            .iter()
            .map(|p| {
                let mut type_name =
                    p.rust_type
                        .to_csharp_string(options, aliases, false, method_name, &p.name);
                if type_name == "bool" {
                    type_name = "[MarshalAs(UnmanagedType.U1)] bool".to_string();
                }

                format!("{} {}", type_name, escape_csharp_name(p.name.as_str()))
            })
            .collect::<Vec<_>>()
            .join(", ");

        if let Some(x) = escape_doc_comment(&item.doc_comment, "        ") {
            method_list_string.push_str_ln(&x);
        }

        method_list_string.push_str_ln(
            format!("        [DllImport(__DllName, EntryPoint = \"{entry_point}\", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]").as_str(),
        );
        if return_type == "bool" {
            method_list_string.push_str_ln("        [return: MarshalAs(UnmanagedType.U1)]");
        }
        method_list_string.push_str_ln(
            format!("        {accessibility} static extern {return_type} {method_prefix}{method_name}({parameters});").as_str(),
        );
        method_list_string.push('\n');
    }

    let mut structs_string = String::new();
    for item in structs {
        let name = (options.csharp_type_rename)(escape_csharp_name(&item.struct_name));
        let layout_kind = if item.is_union {
            "Explicit"
        } else {
            "Sequential"
        };

        if let Some(doc_comment) = escape_doc_comment(&item.doc_comment, "    ") {
            structs_string.push_str_ln(&doc_comment);
        }

        structs_string
            .push_str_ln(format!("    [StructLayout(LayoutKind.{layout_kind})]").as_str());
        structs_string
            .push_str_ln(format!("    {accessibility} unsafe partial struct {name}").as_str());
        structs_string.push_str_ln("    {");
        for field in &item.fields {
            if let Some(doc_comment) = escape_doc_comment(&field.doc_comment, "        ") {
                structs_string.push_str_ln(&doc_comment);
            }
            if item.is_union {
                structs_string.push_str_ln("        [FieldOffset(0)]");
            }

            let type_name = field.rust_type.to_csharp_string(
                options,
                aliases,
                true,
                &"".to_string(),
                &"".to_string(),
            );
            let attr = if type_name == "bool" {
                "[MarshalAs(UnmanagedType.U1)] ".to_string()
            } else {
                "".to_string()
            };

            structs_string.push_str(
                format!(
                    "        {}public {} {}",
                    attr,
                    type_name,
                    escape_csharp_name(field.name.as_str())
                )
                .as_str(),
            );

            if let TypeKind::FixedArray(digits, _) = &field.rust_type.type_kind {
                let mut digits = digits.clone();
                if digits == "0" {
                    digits = "1".to_string(); // 0 fixed array is not allowed in C#
                };

                structs_string.push_str(format!("[{}]", digits).as_str());
            } else {
                let alias_resolved_field =
                    match aliases.get_mapped_value(&field.rust_type.type_name) {
                        Some(x) => x,
                        None => field.rust_type.clone(),
                    };

                if let TypeKind::FixedArray(digits, _) = &alias_resolved_field.type_kind {
                    let mut digits = digits.clone();
                    if digits == "0" {
                        digits = "1".to_string(); // 0 fixed array is not allowed in C#
                    };

                    structs_string.push_str(format!("[{}]", digits).as_str());
                }
            }

            structs_string.push_str_ln(";");
        }
        structs_string.push_str_ln("    }");
        structs_string.push('\n');
    }

    let mut enum_string = String::new();
    for item in enums {
        if let Some(doc_comment) = escape_doc_comment(&item.doc_comment, "    ") {
            enum_string.push_str_ln(&doc_comment);
        }

        let repr = match &item.repr {
            Some(x) => format!(" : {}", convert_token_enum_repr(x)),
            None => "".to_string(),
        };
        let name = (options.csharp_type_rename)(escape_csharp_name(&item.enum_name));
        if item.is_flags {
            enum_string.push_str_ln("    [Flags]");
        }
        enum_string.push_str_ln(format!("    {accessibility} enum {name}{repr}").as_str());
        enum_string.push_str_ln("    {");
        for field in &item.fields {
            if let Some(doc_comment) = escape_doc_comment(&field.doc_comment, "        ") {
                enum_string.push_str_ln(&doc_comment);
            }

            let value = match &field.value {
                Some(x) => format!(" = {x},"),
                None => ",".to_string(),
            };
            enum_string.push_str_ln(format!("        {}{}", field.name, value).as_str());
        }
        enum_string.push_str_ln("    }");
        enum_string.push('\n');
    }

    let mut const_string: String = String::new();
    for item in consts {
        if let Some(doc_comment) = escape_doc_comment(&item.doc_comment, "        ") {
            const_string.push_str_ln(&doc_comment);
        }

        let mut type_name = item.rust_type.to_csharp_string(
            options,
            aliases,
            false,
            &"".to_string(),
            &"".to_string(),
        );

        // special case for string, char, ByteStr
        if item.value.starts_with("\"") {
            type_name = "string".to_string();
        } else if item.value.starts_with("\'") {
            type_name = "char".to_string();
        }

        if item.value.starts_with("[") {
            const_string.push_str(
                format!(
                    "        {} static ReadOnlySpan<byte> {} => new byte[] {};\n",
                    accessibility,
                    escape_csharp_name(item.const_name.as_str()),
                    item.value.replace("[", "{ ").replace("]", " }")
                )
                .as_str(),
            );
        } else {
            let value = if type_name == "float" {
                format!("{}f", item.value)
            } else {
                item.value.to_string()
            };

            const_string.push_str(
                format!(
                    "        {} const {} {} = {};\n",
                    accessibility,
                    type_name,
                    escape_csharp_name(item.const_name.as_str()),
                    value
                )
                .as_str(),
            );
        }
    }

    // use empty string if the generated class is empty.
    let class_string = if method_list_string.is_empty() && const_string.is_empty() {
        String::new()
    } else {
        format!(
            "{accessibility} static unsafe partial class {class_name}
    {{
{dll_name}

{const_string}

{method_list_string}
    }}"
        )
    };

    let file_header = if !options.csharp_file_header.is_empty() {
        options.csharp_file_header.to_string() + "\n"
    } else {
        "".to_string()
    } + "// <auto-generated>
// This code is generated by csbindgen.
// DON'T CHANGE THIS DIRECTLY.
// </auto-generated>
#pragma warning disable CS8500
#pragma warning disable CS8981";
    let file_footer = &options.csharp_file_footer;

    let mut imported_namespaces = String::new();
    for name in &options.csharp_imported_namespaces {
        imported_namespaces.push_str_ln(format!("using {name};").as_str());
    }

    let result = format!(
        "{file_header}
using System;
using System.Runtime.InteropServices;
{imported_namespaces}

namespace {namespace}
{{
    {class_string}

{structs_string}
{enum_string}
}}
{file_footer}"
    );

    result
}

fn convert_token_enum_repr(repr: &str) -> &str {
    match repr {
        "(C)" => "uint",
        "(u8)" => "byte",
        "(u16)" => "ushort",
        "(u32)" => "uint",
        "(u64)" => "ulong",
        "(i8)" => "sbyte",
        "(i16)" => "short",
        "(i32)" => "int",
        "(i64)" => "long",
        "C" => "uint",
        "u8" => "byte",
        "u16" => "ushort",
        "u32" => "uint",
        "u64" => "ulong",
        "i8" => "sbyte",
        "i16" => "short",
        "i32" => "int",
        "i64" => "long",
        x => x,
    }
}
