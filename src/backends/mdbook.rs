use crate::{config::*, document::*, ensure_dir, read_file};
use fs_extra::{copy_items, dir::CopyOptions};
use regex::{Captures, Regex};
use serde::Serialize;
use std::{
    collections::HashMap,  fs::{remove_dir_all, write}, path::Path, process::Command
};

#[derive(Serialize)]
struct Book {
    pub book: BookInner,
    pub output: BookOutput
}

#[derive(Serialize)]
struct BookInner {
    pub authors: Vec<String>,
    pub language: String,
    pub multilingual: bool,
    pub src: String,
    pub title: String,
}

#[derive(Serialize)]
pub struct BookOutput {
    html: BookHtml,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BookHtml {
    default_theme: String,
    preferred_dark_theme: String,
    mathjax_support: bool,
    no_section_label: bool,
    site_url: String,
    fold: BookFold,
}

#[derive(Serialize)]
pub struct BookFold {
    enable: bool,
    level: usize,
}

pub fn bake_mdbook(document: &Document, config: &Config, root: &Path) {
    let cleanup = config
        .backend_mdbook
        .as_ref()
        .map(|mdbook| mdbook.cleanup)
        .unwrap_or_default();
    if cleanup {
        let _ = remove_dir_all(&config.output_dir);
    }

    write_manifest(config);

    let mut files = HashMap::new();
    let mut index = "# Index\n\n".to_owned();

    index.push_str("[Documentation](documentation.md)\n");
    let mut documentation = if let Some(content) = document.book.get("documentation.md") {
        format!("{}\n\n", content)
    } else {
        String::new()
    };
    documentation.push_str("# Contents\n");

    if document.book.keys().any(|k| k == "index.txt") {
        include_book_index(None, &document.book, &mut files, &mut index, 0);
    }

    index.push_str("\n- [C++ API Reference](reference.md)\n");
    let mut reference_listing = "# C++ API Reference\n".to_owned();
    documentation.push_str("- [C++ API Reference](reference.md)\n");

    if !document.enums.is_empty() {
        index.push_str("  - [Enums](reference/enums.md)\n");
        reference_listing.push_str("\n## Enums\n");
        let mut listing = "# Enums\n\n".to_owned();
        for item in &document.enums {
            let index_path = format!("reference/enums/{}.md", item.name);
            let file_path = format!("src/reference/enums/{}.md", item.name);
            let mut content = String::default();
            bake_enum(item, &mut content);
            files.insert(file_path, content);
            let entry = format!("    - [{}]({})\n", item.name, index_path);
            index.push_str(&entry);
            let entry = format!("- [`{}`]({})\n", item.name, index_path);
            listing.push_str(&entry);
            reference_listing.push_str(&entry);
        }
        files.insert("src/reference/enums.md".to_owned(), listing);
    }

    if !document.structs.is_empty() {
        index.push_str("  - [Structs](reference/structs.md)\n");
        reference_listing.push_str("\n## Structs\n");
        let mut listing = "# Structs\n\n".to_owned();
        for item in &document.structs {
            let index_path = format!("reference/structs/{}.md", item.name);
            let file_path = format!("src/reference/structs/{}.md", item.name);
            let mut content = String::default();
            bake_struct_class(item, &mut content);
            files.insert(file_path, content);
            let entry = format!("    - [{}]({})\n", item.name, index_path);
            index.push_str(&entry);
            let entry = format!("- [`{}`]({})\n", item.name, index_path);
            listing.push_str(&entry);
            reference_listing.push_str(&entry);
        }
        files.insert("src/reference/structs.md".to_owned(), listing);
    }

    if !document.classes.is_empty() {
        index.push_str("  - [Classes](reference/classes.md)\n");
        reference_listing.push_str("\n## Classes\n");
        let mut listing = "# Classes\n\n".to_owned();
        for item in &document.classes {
            let index_path = format!("reference/classes/{}.md", item.name);
            let file_path = format!("src/reference/classes/{}.md", item.name);
            let mut content = String::default();
            bake_struct_class(item, &mut content);
            files.insert(file_path, content);
            let entry = format!("    - [{}]({})\n", item.name, index_path);
            index.push_str(&entry);
            let entry = format!("- [`{}`]({})\n", item.name, index_path);
            listing.push_str(&entry);
            reference_listing.push_str(&entry);
        }
        files.insert("src/reference/classes.md".to_owned(), listing);
    }

    if !document.functions.is_empty() {
        index.push_str("  - [Functions](reference/functions.md)\n");
        reference_listing.push_str("\n## Functions\n");
        let mut listing = "# Functions\n\n".to_owned();
        for item in &document.functions {
            let index_path = format!("reference/functions/{}.md", item.name);
            let file_path = format!("src/reference/functions/{}.md", item.name);
            let mut content = String::default();
            bake_function(item, &mut content, false);
            files.insert(file_path, content);
            let entry = format!("    - [{}]({})\n", item.name, index_path);
            index.push_str(&entry);
            let entry = format!("- [`{}`]({})\n", item.name, index_path);
            listing.push_str(&entry);
            reference_listing.push_str(&entry);
        }
        files.insert("src/reference/functions.md".to_owned(), listing);
    }

    if !document.delegates.is_empty() {
        index.push_str("  - [Delegates](reference/delegates.md)\n");
        reference_listing.push_str("\n## Delegates\n");
        let mut listing = "# Delegates\n\n".to_owned();
        for item in &document.delegates {
            let index_path = format!("reference/delegates/{}.md", item.name);
            let file_path = format!("src/reference/delegates/{}.md", item.name);
            let mut content = String::default();
            bake_delegate(item, &mut content); // write out delegate content
            files.insert(file_path, content);
            let entry = format!("    - [{}]({})\n", item.name, index_path);
            index.push_str(&entry);
            let entry = format!("- [`{}`]({})\n", item.name, index_path);
            listing.push_str(&entry);
            reference_listing.push_str(&entry);
        }
        files.insert("src/reference/delegates.md".to_owned(), listing);
    }

    files.insert("src/reference.md".to_owned(), reference_listing);
    files.insert("src/documentation.md".to_owned(), documentation);

    let header = config
        .backend_mdbook
        .as_ref()
        .and_then(|mdbook| mdbook.header.as_ref())
        .map(|path| {
            read_file(root.join(path))
                .unwrap_or_else(|_| panic!("Could not read header file: {:?}", path))
                + "\n"
        })
        .unwrap_or_default();
    let footer = config
        .backend_mdbook
        .as_ref()
        .and_then(|mdbook| mdbook.footer.as_ref())
        .map(|path| {
            "\n".to_owned()
                + &read_file(root.join(path))
                    .unwrap_or_else(|_| panic!("Could not read footer file: {:?}", path))
        })
        .unwrap_or_default();
    for (path, content) in files {
        let relative_path = if path.starts_with("src/") {
            path.rfind('/')
                .map(|index| &path[4..(index + 1)])
                .unwrap_or_else(|| "")
        } else {
            ""
        };
        let content = preprocess_content(
            &content,
            document,
            config.backend_mdbook.as_ref(),
            relative_path,
        );
        let path = config.output_dir.join(path);
        ensure_dir(&path);
        let content = format!(
            "{}{}{}\n",
            header,
            content,
            footer,
        );
        write(&path, content)
            .unwrap_or_else(|_| panic!("Could not write mdbook page file: {:?}", path));
    }

    let path = config.output_dir.join("src/SUMMARY.md");
    ensure_dir(&path);
    write(&path, index)
        .unwrap_or_else(|_| panic!("Could not write mdbook summary file: {:?}", path));

    if let Some(assets) = config
        .backend_mdbook
        .as_ref()
        .and_then(|mdbook| mdbook.assets.as_ref())
    {
        let from = root.join(assets);
        let to = config.output_dir.join("src/assets");
        ensure_dir(&to);
        let mut options = CopyOptions::new();
        options.overwrite = true;
        options.copy_inside = true;
        copy_items(&[from], &to, &options)
            .unwrap_or_else(|_| panic!("Could not copy assets: {:?}", assets));
    }

    let build = config
        .backend_mdbook
        .as_ref()
        .map(|mdbook| mdbook.build)
        .unwrap_or_default();
    if build {
        Command::new("mdbook")
            .arg("build")
            .arg(&config.output_dir)
            .status()
            .expect("Could not build documentation with mdbook!");
    }
}

fn preprocess_content(
    content: &str,
    document: &Document,
    config: Option<&BackendMdBook>,
    relative_path: &str,
) -> String {
    let content = replace_code_references(content, document);
    let content = replace_snippets(&content, document);
    fix_site_references(
        &content,
        config
            .and_then(|config| config.site_url.as_deref())
            .unwrap_or("/"),
        relative_path,
    )
}

fn replace_code_references(content: &str, document: &Document) -> String {
    // TODO: put that regex in lazy static to not perform costly compilation on each call.
    let re = Regex::new(r"\[`\s*(\w+)\s*:\s*(\w+)\s*(::\s*(\w+))?`\]s*\(\s*\)").unwrap();
    re.replace_all(content, |captures: &Captures| {
        let element = captures.get(1).unwrap().as_str().trim();
        let name = captures.get(2).unwrap().as_str().trim();
        let section = captures.get(4).map(|m| m.as_str().trim());
        let path = match element {
            "enum" => document
                .enums
                .iter()
                .find(|item| item.name == name)
                .map(|_| format!("/reference/enums/{}.md", name)),
            "struct" => document
                .structs
                .iter()
                .find(|item| item.name == name)
                .map(|_| format!("/reference/structs/{}.md", name)),
            "class" => document
                .classes
                .iter()
                .find(|item| item.name == name)
                .map(|_| format!("/reference/classes/{}.md", name)),
            "function" => document
                .functions
                .iter()
                .find(|item| item.name == name)
                .map(|_| format!("/reference/functions/{}.md", name)),
            "delegate" => document
                .delegates
                .iter()
                .find(|item| item.name == name)
                .map(|_| format!("/reference/delegates/{}.md", name)),
            _ => None,
        };
        if let Some(path) = path {
            if let Some(section) = section {
                format!(
                    "[**`{}::{}`**]({}#{})",
                    name,
                    section,
                    path,
                    section.to_lowercase()
                )
            } else {
                format!("[**`{}`**]({})", name, path)
            }
        } else if let Some(section) = section {
            format!("**`{}::{}`**", name, section)
        } else {
            format!("**`{}`**", name)
        }
    })
    .into()
}

fn replace_snippets(content: &str, document: &Document) -> String {
    // TODO: put that regex in lazy static to not perform costly compilation on each call.
    let re = Regex::new(r"```\s*snippet[\n\r]+([\s/]*)(\w+)[\r\n]+\s*```").unwrap();
    re.replace_all(content, |captures: &Captures| {
        let prefix = captures.get(1).unwrap().as_str();
        let name = captures.get(2).unwrap().as_str().trim();
        if let Some(content) = document.snippets.get(name) {
            let content = content
                .lines()
                .map(|line| format!("{}{}", prefix, line))
                .collect::<Vec<_>>()
                .join("\n");
            format!("```cpp\n{}\n{}```", content, prefix)
        } else {
            println!("Trying to inject non-existing snippet: {}", name);
            format!("```\n{}Missing snippet: {}\n{}```", prefix, name, prefix)
        }
    })
    .into()
}

fn fix_site_references(content: &str, site_url: &str, relative_path: &str) -> String {
    // TODO: put that regex in lazy static to not perform costly compilation on each call.
    let re = Regex::new(r"\]\s*\((\s*/)?(.*\.md(\s*#.*)?)\)").unwrap();
    re.replace_all(content, |captures: &Captures| {
        let relative_path = captures.get(1).map(|_| "").unwrap_or_else(|| relative_path);
        let reference = captures.get(2).unwrap().as_str().trim();
        format!("]({}{}{})", site_url, relative_path, reference)
    })
    .into()
}

fn include_book_index(
    dir: Option<&str>,
    input_files: &HashMap<String, String>,
    output_files: &mut HashMap<String, String>,
    index: &mut String,
    level: usize,
) {
    let path = dir.map(|v| format!("{}/", v)).unwrap_or_default();
    if let Some(content) = input_files.get(&format!("{}index.txt", path)) {
        let mut listing = if let Some(content) = input_files.get(&format!("{}index.md", path)) {
            format!("{}\n\n", content)
        } else {
            String::new()
        };
        listing.push_str("# Pages\n\n");
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                let (name, title) = if let Some(position) = line.find(':') {
                    (line[..position].trim(), Some(line[(position + 1)..].trim()))
                } else {
                    (line, None)
                };
                index.push_str(&"  ".repeat(level));
                let path = format!("{}{}", path, name);
                if name.ends_with(".md") {
                    if let Some(content) = input_files.get(&path) {
                        let title = title
                            .or_else(|| {
                                content
                                    .lines()
                                    .next()
                                    .map(|line| line.trim_start_matches('#').trim())
                            })
                            .unwrap_or(name);
                        index.push_str(&format!("- [{}]({})\n", title, path));
                        output_files.insert(format!("src/{}", path), content.to_owned());
                        listing.push_str(&format!("- [{}]({})\n", title, name));
                    }
                } else {
                    index.push_str(&format!(
                        "- [{}]({}/index.md)\n",
                        title.unwrap_or(name),
                        path
                    ));
                    include_book_index(Some(&path), input_files, output_files, index, level + 1);
                }
            }
        }
        output_files.insert(format!("src/{}index.md", path), listing);
    }
}

fn bake_specifiers(specifiers: &Specifiers, content: &mut String) {
    content.push_str("**_Reflection-enabled_**\n");
    if !specifiers.attributes.is_empty() {
        content.push_str("\n### Specifiers:\n");
        for attribute in &specifiers.attributes {
            match attribute {
                Attribute::Single(name) => {
                    content.push_str(&format!("- **{}**\n", name));
                }
                Attribute::Pair { key, value } => {
                    content.push_str(&format!("- **{}** = _{}_\n", key, value));
                }
            }
        }
    }
    if !specifiers.meta.is_empty() {
        content.push_str("\n### Meta Specifiers:\n");
        for attribute in &specifiers.meta {
            match attribute {
                Attribute::Single(name) => {
                    content.push_str(&format!("- **{}**\n", name));
                }
                Attribute::Pair { key, value } => {
                    content.push_str(&format!("- **{}** = _{}_\n", key, value));
                }
            }
        }
    }
    content.push('\n');
}

fn bake_enum(item: &Enum, content: &mut String) {
    content.push_str(&format!("# **Enum: `{}`**\n\n", item.name));
    content.push_str(&format!("```cpp\n//  {} : {}\n\n{}\n```\n\n", item.filename, item.fileline, item.signature()));
    if let Some(specifiers) = &item.specifiers {
        content.push_str("---\n\n");
        bake_specifiers(specifiers, content);
    }
    content.push_str("---\n\n");
    content.push_str(&item.doc_comments.to_owned().unwrap_or_default());
    content.push_str("\n\n");
}

fn bake_struct_class(item: &StructClass, content: &mut String) {
    match item.mode {
        StructClassMode::Struct => content.push_str(&format!("# **Struct: `{}`**\n\n", item.name)),
        StructClassMode::Class => content.push_str(&format!("# **Class: `{}`**\n\n", item.name)),
    }
    content.push_str(&format!("```cpp\n//  {} : {}\n\n{}\n```\n\n", item.filename, item.fileline, item.signature()));
    if let Some(specifiers) = &item.specifiers {
        content.push_str("---\n\n");
        bake_specifiers(specifiers, content);
    }
    content.push_str("---\n\n");
    bake_struct_class_comments(&item, content);
    content.push_str("\n\n");
    if !item.properties.is_empty() {
        content.push_str("---\n\n# **Properties**\n\n");
        for property in &item.properties {
            bake_property(property, content, true);
        }
        content.push_str("\n\n");
    }
    if !item.methods.is_empty() {
        content.push_str("---\n\n# **Methods**\n\n");
        for method in &item.methods {
            bake_function(method, content, true);
        }
        content.push_str("\n\n");
    }
}

fn bake_struct_class_comments(item: &StructClass, content: &mut String) {
    if let Some(comments) = item.doc_comments.to_owned() {
        let re = Regex::new(r"(?ms).*<summary>(.*)</summary>.*").unwrap();
        if let Some(caps) = re.captures(comments.as_str()) {
            if caps.len() > 1 {
                content.push_str(&caps[1]);
            }
        } else {
            content.push_str(&comments);
        }
    }
}

fn bake_property(item: &Property, content: &mut String, member: bool) {
    let level = if member {
        content.push_str(&format!("* # __`{}`__\n\n", item.name));
        4
    } else {
        content.push_str(&format!("# **Property: `{}`**\n\n", item.name));
        0
    };
    let indented = indent(level, &{
        let mut content = String::default();
        content.push_str(&format!("```cpp\n{}\n```\n\n", item.signature()));
        if let Some(specifiers) = &item.specifiers {
            content.push_str("---\n\n");
            bake_specifiers(specifiers, &mut content);
        }
        content.push_str("---\n\n");
        content.push_str(&item.doc_comments.to_owned().unwrap_or_default());
        content.push_str("\n\n");
        content
    });
    content.push_str(&indented);
    content.push_str("\n\n");
}

fn bake_delegate(item: &Delegate, content: &mut String) {
    content.push_str(&format!("# **Delegate: `{}`**\n\n", item.name));

    // declaration
    content.push_str(&format!("```cpp\n// Delegate type\n{}\n\n// Compatible function signtature\n{}\n\n```\n\n", item.signature(), item.callback_signature()));
    // content.push_str("```cpp\n{}\n```\n\n", item.);

    // UDELEGATE specifiers
    if let Some(specifiers) = &item.specifiers {
        content.push_str("---\n\n");
        bake_specifiers(specifiers, content);
    }

    // main comments
    bake_delegate_comments(&item.doc_comments, content);

    // individual args
    if !item.arguments.is_empty() {
        content.push_str("---\n\n# **Parameters**\n\n");
        for arg in &item.arguments {
            bake_delegate_argument(arg, &item.doc_comments, content);
        }
    }

    // return value
    bake_delegate_return_type(&item.return_type, &item.doc_comments, content);

    content.push_str("\n\n");
}

fn bake_delegate_comments(doc_comments: &Option<String>, content: &mut String) {
    if let Some(comments) = doc_comments {
        let re = Regex::new(r"(?ms).*<summary>(.*)</summary>.*").unwrap();
        if let Some(caps) = re.captures(comments) {
            if caps.len() > 1 {
                content.push_str(format!("<summary>\n\n{}</summary>", &caps[1]).as_str());
            }
        } else {
            content.push_str(&comments);
        }
    }
}

fn bake_delegate_argument(item: &Argument, fun_comments: &Option<String>, content: &mut String) {
    if let Some(name) = &item.name {
        content.push_str(&format!("* ## __`{}`__\n\n", name));
    } else {
        let re = Regex::new(r"(?ms)/\*(.*)\*/").unwrap();
        if let Some(caps) = re.captures(&item.signature()) {
            if caps.len() > 1 {
                content.push_str(format!("* ## __`{}`__\n\n", &caps[1]).as_str());
            }
        }
        else {
            content.push_str("* _Unnamed_\n\n");
        }
    }
    let indented = indent(4, &{
        let mut content = String::default();
        content.push_str(&format!("```cpp\n{}\n```\n\n", item.signature()));
        content.push_str(&item.doc_comments.to_owned().unwrap_or_default());

        if let Some(comments) = fun_comments {
            if let Some(name) = &item.name {
                let re = Regex::new(format!(r#"<param name=\"{}\">(.*)</param>"#, &name).as_str()).unwrap();
                if let Some(caps) = re.captures(comments) {
                    if caps.len() > 1 {
                        content.push_str("\n\n");
                        content.push_str(&caps[1]);
                    }
                }
            }
        }

        content.push_str("\n\n");
        content
    });
    content.push_str(&indented);
    content.push_str("\n\n");
}

fn bake_delegate_return_type(return_type: &Option<String>, doc_comments: &Option<String>, content: &mut String) {
    if let Some(r) = return_type {
        if r != "void" {
            content.push_str("---\n\n# **Returns**\n\n");
            content.push_str("*\n");
            let indented = indent(4, &{
                let mut content = String::default();

                content.push_str(&format!("```cpp\n{}\n```\n\n", r));

                if let Some(comments) = doc_comments {
                    let re = Regex::new(r"<returns>(.*)</returns>").unwrap();
                    if let Some(caps) = re.captures(comments) {
                        if caps.len() > 1 {
                            content.push_str(&caps[1]);
                        }
                    }
                }
                content.push_str("\n\n");
                content
            });
            content.push_str(&indented);
            content.push_str("\n\n");
        }
    }
}

fn bake_function(item: &Function, content: &mut String, member: bool) {
    let level = if member {
        content.push_str(&format!("* # __`{}`__\n\n", item.name));
        4
    } else {
        content.push_str(&format!("# **Function: `{}`**\n\n", item.name));
        0
    };
    let indented = indent(level, &{
        let mut content = String::default();
        content.push_str(&format!("```cpp\n//  {} : {}\n\n{}\n```\n\n", item.filename, item.fileline, item.signature()));
        if member {
            content.push_str("<details>\n\n");
        }
        if let Some(specifiers) = &item.specifiers {
            content.push_str("---\n\n");
            bake_specifiers(specifiers, &mut content);
        }

        bake_function_comments(&item.doc_comments, &mut content);

        content.push_str("\n\n");
        if !item.arguments.is_empty() {
            content.push_str("---\n\n# **Arguments**\n\n");
            for argument in &item.arguments {
                bake_function_argument(argument, &item.doc_comments, &mut content);
            }
            content.push_str("\n\n");
        }

        bake_function_return_type(&item.return_type, &item.doc_comments, &mut content);

        if member {
            content.push_str("</details>\n\n");
        }
        content
    });
    content.push_str(&indented);
    content.push_str("\n\n");
}

fn bake_function_argument(item: &Argument, fun_comments: &Option<String>, content: &mut String) {
    if let Some(name) = &item.name {
        content.push_str(&format!("* ## __`{}`__\n\n", name));
    } else {
        content.push_str("* _Unnamed_\n\n");
    }
    let indented = indent(4, &{
        let mut content = String::default();
        content.push_str(&format!("```cpp\n{}\n```\n\n", item.signature()));
        content.push_str(&item.doc_comments.to_owned().unwrap_or_default());

        if let Some(comments) = fun_comments {
            if let Some(name) = &item.name {
                let re = Regex::new(format!(r#"<param name=\"{}\">(.*)</param>"#, &name).as_str()).unwrap();
                if let Some(caps) = re.captures(comments) {
                    if caps.len() > 1 {
                        content.push_str("\n\n");
                        content.push_str(&caps[1]);
                    }
                }
            }
        }

        content.push_str("\n\n");
        content
    });
    content.push_str(&indented);
    content.push_str("\n\n");
}

fn bake_function_return_type(return_type: &Option<String>, doc_comments: &Option<String>, content: &mut String) {
    if let Some(r) = return_type {
        if r != "void" {
            content.push_str("---\n\n# **Returns**\n\n");
            content.push_str("*\n");
            let indented = indent(4, &{
                let mut content = String::default();

                content.push_str(&format!("```cpp\n{}\n```\n\n", r));

                if let Some(comments) = doc_comments {
                    let re = Regex::new(r"<returns>(.*)</returns>").unwrap();
                    if let Some(caps) = re.captures(comments) {
                        if caps.len() > 1 {
                            content.push_str(&caps[1]);
                        }
                    }
                }
                content.push_str("\n\n");
                content
            });
            content.push_str(&indented);
            content.push_str("\n\n");
        }
    }
}

fn bake_function_comments(doc_comments: &Option<String>, content: &mut String) {
    if let Some(comments) = doc_comments {
        let re = Regex::new(r"(?ms).*<summary>(.*)</summary>.*").unwrap();
        if let Some(caps) = re.captures(comments) {
            if caps.len() > 1 {
                content.push_str(format!("<summary>\n\n{}</summary>", &caps[1]).as_str());
            }
        } else {
            content.push_str(&comments);
        }
    }
}

fn indent(level: usize, content: &str) -> String {
    if level > 0 {
        content
            .lines()
            .map(|line| " ".repeat(level) + line)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        content.to_owned()
    }
}

fn write_manifest(config: &Config) {
    let mdbook = config.backend_mdbook.as_ref().cloned().unwrap_or_default();

    let manifest = Book {
        book: BookInner {
            authors: mdbook.authors.to_owned(),
            language: mdbook.language.to_owned(),
            multilingual: mdbook.multilingual,
            src: "src".to_owned(),
            title: mdbook.title.to_owned(),
        },
        output: BookOutput {
            html: BookHtml {
                default_theme: "ayu".to_owned(),
                preferred_dark_theme: "ayu".to_owned(),
                mathjax_support: true,
                no_section_label: true,
                site_url: mdbook.site_url.unwrap_or("/".to_string()),
                fold: BookFold {
                    enable: false,
                    level: 0,
                },
            },
        }
    };

    let content = toml::to_string(&manifest).expect("Could not serialize mdbook manifest!");
    let path = config.output_dir.join("book.toml");
    ensure_dir(&path);
    write(&path, content)
        .unwrap_or_else(|_| panic!("Could not write mdbook manifest file: {:?}", path));
}
