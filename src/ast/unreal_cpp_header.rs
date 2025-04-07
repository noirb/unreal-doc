use crate::{config::Settings, document::*};
use pest::{error::Error, iterators::Pair, Parser};
use std::collections::HashSet;
use std::path::Path;

#[derive(Parser)]
#[grammar = "ast/unreal_cpp_header.pest"]
pub struct UnrealCppHeaderParser;

#[allow(clippy::result_large_err)]
pub fn parse_unreal_cpp_header(
    content: &str,
    document: &mut Document,
    settings: &Settings,
    path: &Path
) -> Result<(), Error<Rule>> {
    let pair = UnrealCppHeaderParser::parse(Rule::file, content)?
        .next()
        .unwrap();
    if pair.as_rule() == Rule::file {
        let filename = path.file_name().unwrap().to_str().unwrap();
        parse_file(pair, document, settings, filename);
    }
    Ok(())
}

fn parse_unreal_cpp_element(
    content: &str,
    document: &mut Document,
    settings: &Settings,
    filename: &str
) -> Element {
    let pair = UnrealCppHeaderParser::parse(Rule::element, content)
        .unwrap_or_else(|error| {
            panic!(
                "Could not parse Unreal C++ element content!\nError:\n{}",
                error
            )
        })
        .next()
        .unwrap();
    match pair.as_rule() {
        Rule::element => parse_element(pair, Visibility::Public, settings, document, filename),
        _ => unreachable!(),
    }
}

fn parse_file(pair: Pair<Rule>, document: &mut Document, settings: &Settings, filename: &str) {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::proxy => parse_proxy(pair, settings, document, filename),
            Rule::snippet => parse_snippet(pair, document),
            Rule::element => match parse_element(pair, Visibility::Public, settings, document, filename) {
                Element::Enum(element) => {
                    if element.can_export(settings) {
                        if document.enums.iter().any(|item| item.name == element.name) {
                            println!("Overwriting existing enum: {}", element.name);
                        }
                        document.enums.push(element)
                    }
                }
                Element::StructClass(element) => match element.mode {
                    StructClassMode::Struct => {
                        if element.can_export(settings) {
                            if document
                                .structs
                                .iter()
                                .any(|item| item.name == element.name)
                            {
                                println!("Overwriting existing struct: {}", element.name);
                            }
                            document.structs.push(element)
                        }
                    }
                    StructClassMode::Class => {
                        if element.can_export(settings) {
                            if document
                                .classes
                                .iter()
                                .any(|item| item.name == element.name)
                            {
                                println!("Overwriting existing class: {}", element.name);
                            }
                            document.classes.push(element)
                        }
                    }
                },
                Element::Delegate(element) => {
                    if element.can_export(settings) {
                        if document
                            .delegates
                            .iter()
                            .any(|item| item.name == element.name)
                        {
                            println!("Overwriting existing delegate: {}", element.name);
                        }
                        document.delegates.push(element);
                    }
                },
                Element::Function(element) => {
                    if element.can_export(settings) {
                        if document
                            .functions
                            .iter()
                            .any(|item| item.name == element.name)
                        {
                            println!("Overwriting existing function: {}", element.name);
                        }
                        document.functions.push(element)
                    }
                },
                _ => {}
            },
            _ => {}
        }
    }
}

fn parse_proxy(pair: Pair<Rule>, settings: &Settings, document: &mut Document, filename: &str) {
    let mut doc_comments = None;
    let mut tags = HashSet::new();
    let mut content = String::new();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::doc_comment_lines => doc_comments = Some(parse_doc_comments(pair)),
            Rule::proxy_tags => {
                for pair in pair.into_inner() {
                    tags.insert(parse_identifier(pair));
                }
            }
            Rule::proxy_line_content => content.push_str(pair.as_str()),
            _ => {}
        }
    }
    match parse_unreal_cpp_element(&content, document, settings, filename) {
        Element::Function(mut item) => {
            if let Some(doc_comments) = doc_comments {
                item.doc_comments = Some(doc_comments);
                document.proxy_functions.push(Proxy { tags, item });
            }
        }
        Element::Property(mut item) => {
            if let Some(doc_comments) = doc_comments {
                item.doc_comments = Some(doc_comments);
                document.proxy_properties.push(Proxy { tags, item });
            }
        }
        _ => {}
    }
}

fn parse_snippet(pair: Pair<Rule>, document: &mut Document) {
    let mut id = None;
    let mut content = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::identifier => id = Some(parse_identifier(pair)),
            Rule::snippet_inner => content = Some(parse_snippet_inner(pair)),
            _ => {}
        }
    }
    if let (Some(id), Some(content)) = (id, content) {
        if document.snippets.contains_key(&id) {
            println!("Overwriting existing snippet: {}", id);
        }
        document.snippets.insert(id, content);
    }
}

fn parse_snippet_inner(pair: Pair<Rule>) -> String {
    let level = pair
        .as_str()
        .lines()
        .map(|line| line.chars().take_while(|c| c.is_whitespace()).count())
        .min_by(|a, b| a.cmp(b))
        .unwrap_or_default();
    pair.as_str()
        .lines()
        .map(|line| line[level..].to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_doc_comments(pair: Pair<Rule>) -> String {
    pair.as_str()
        .lines()
        .map(|line| {
            line.find("///")
                .map(|loc| line[(loc + 3)..].trim().to_owned())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

enum Element {
    None,
    Enum(Enum),
    StructClass(StructClass),
    Property(Property),
    Function(Function),
    Delegate(Delegate),
}

fn parse_element(
    pair: Pair<Rule>,
    visibility: Visibility,
    settings: &Settings,
    document: &mut Document,
    filename: &str
) -> Element {
    let mut result = Element::None;
    let mut doc_comments = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::doc_comment_lines => doc_comments = Some(parse_doc_comments(pair)),
            Rule::element_enum => result = Element::Enum(parse_element_enum(pair, &doc_comments, filename)),
            Rule::element_struct => {
                result = Element::StructClass(parse_element_struct_class(
                    pair,
                    &doc_comments,
                    StructClassMode::Struct,
                    settings,
                    document,
                    filename
                ));
            }
            Rule::element_class => {
                result = Element::StructClass(parse_element_struct_class(
                    pair,
                    &doc_comments,
                    StructClassMode::Class,
                    settings,
                    document,
                    filename
                ));
            }
            Rule::element_property => {
                result = Element::Property(parse_element_property(pair, &doc_comments, visibility));
            }
            Rule::element_function => {
                result = Element::Function(parse_element_function(
                    pair,
                    &doc_comments,
                    visibility,
                    document,
                    filename
                ));
            }
            Rule::element_delegate => {
                result = Element::Delegate(parse_element_delegate(
                    pair,
                    &doc_comments,
                    document,
                    filename
                ));
            }
            Rule::element_multicast_delegate => {
                result = Element::Delegate(parse_element_delegate(
                    pair,
                    &doc_comments,
                    document,
                    filename
                ));
            }
            Rule::element_dynamic_delegate => {
                result = Element::Delegate(parse_element_delegate(
                    pair,
                    &doc_comments,
                    document,
                    filename
                ));
            }
            Rule::element_dyn_multicast_delegate => {
                result = Element::Delegate(parse_element_delegate(
                    pair,
                    &doc_comments,
                    document,
                    filename
                ));
            }
            _ => {}
        }
    }
    result
}

fn parse_specifiers(pair: Pair<Rule>) -> Specifiers {
    let mut result = Specifiers::default();
    if let Some(pair) = pair.into_inner().next() {
        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::specifier_single => result.attributes.push(Attribute::Single(
                    parse_identifier(pair.into_inner().next().unwrap()),
                )),
                Rule::specifier_pair => {
                    let mut pairs = pair.into_inner();
                    result.attributes.push(Attribute::Pair {
                        key: parse_identifier(pairs.next().unwrap()),
                        value: parse_identifier(pairs.next().unwrap()),
                    })
                }
                Rule::specifier_meta => parse_specifier_meta(pair, &mut result),
                _ => {}
            }
        }
    }
    result
}

fn parse_specifier_meta(pair: Pair<Rule>, result: &mut Specifiers) {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::specifier_single => result.meta.push(Attribute::Single(parse_identifier(
                pair.into_inner().next().unwrap(),
            ))),
            Rule::specifier_pair => {
                let mut pairs = pair.into_inner();
                result.meta.push(Attribute::Pair {
                    key: parse_identifier(pairs.next().unwrap()),
                    value: parse_identifier(pairs.next().unwrap()),
                })
            }
            _ => {}
        }
    }
}

fn parse_element_enum(pair: Pair<Rule>, doc_comments: &Option<String>, filename: &str) -> Enum {
    let mut result = Enum {
        doc_comments: doc_comments.to_owned(),
        ..Default::default()
    };
    result.fileline = pair.line_col().0;
    result.filename = filename.to_string();

    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::uenum => result.specifiers = Some(parse_specifiers(pair)),
            Rule::enum_signature => result.name = parse_enum_signature(pair),
            Rule::enum_body => parse_enum_body(pair, &mut result),
            _ => {}
        }
    }
    result
}

fn parse_enum_signature(pair: Pair<Rule>) -> String {
    parse_identifier(pair.into_inner().next().unwrap())
}

fn parse_enum_body(pair: Pair<Rule>, result: &mut Enum) {
    for pair in pair.into_inner() {
        result.variants.push(parse_identifier(pair));
    }
}

fn parse_element_struct_class(
    pair: Pair<Rule>,
    doc_comments: &Option<String>,
    mode: StructClassMode,
    settings: &Settings,
    document: &mut Document,
    filename: &str
) -> StructClass {
    let mut result = StructClass {
        mode,
        doc_comments: doc_comments.to_owned(),
        ..Default::default()
    };
    result.filename = filename.to_string();
    result.fileline = pair.line_col().0;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::ustruct | Rule::uclass => result.specifiers = Some(parse_specifiers(pair)),
            Rule::struct_signature | Rule::class_signature => {
                parse_struct_class_signature(pair, &mut result);
            }
            Rule::struct_class_body => {
                parse_struct_class_body(
                    pair,
                    &mut result,
                    mode.default_visibility(),
                    settings,
                    document,
                    filename
                );
            }
            _ => {}
        }
    }
    result
}

fn parse_struct_class_signature(pair: Pair<Rule>, result: &mut StructClass) {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::template_declaration => result.template = Some(parse_template_declaration(pair)),
            Rule::api => result.api = Some(parse_identifier(pair)),
            Rule::identifier => result.name = parse_identifier(pair),
            Rule::inheritances => result.inherits = parse_inheritances(pair),
            _ => {}
        }
    }
}

fn parse_struct_class_body(
    pair: Pair<Rule>,
    result: &mut StructClass,
    mut visibility: Visibility,
    settings: &Settings,
    document: &mut Document,
    filename: &str
) {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::visibility => {
                if let Some(v) = parse_visibility(pair) {
                    visibility = v;
                }
            }
            Rule::inject => {
                for pair in pair.into_inner() {
                    result.injects.insert(parse_identifier(pair));
                }
            }
            Rule::element => match parse_element(pair, visibility, settings, document, filename) {
                Element::Property(element) => {
                    if element.can_export(settings) {
                        result.properties.push(element);
                    }
                }
                Element::Function(element) => {
                    if element.can_export(settings) {
                        if element.return_type == None {
                            result.constructors.push(element);
                        }
                        else {
                            result.methods.push(element);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn parse_element_property(
    pair: Pair<Rule>,
    doc_comments: &Option<String>,
    visibility: Visibility,
) -> Property {
    let mut result = Property {
        doc_comments: doc_comments.to_owned(),
        visibility,
        ..Default::default()
    };
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::uproperty => result.specifiers = Some(parse_specifiers(pair)),
            Rule::property_signature => parse_property_signature(pair, &mut result),
            _ => {}
        }
    }
    result
}

fn parse_property_signature(pair: Pair<Rule>, result: &mut Property) {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::value_type => result.value_type = parse_value_type(pair),
            Rule::identifier => result.name = parse_identifier(pair),
            Rule::property_array => result.array = parse_property_array(pair),
            Rule::default_value => result.default_value = Some(parse_default_value(pair)),
            Rule::staticness => result.is_static = true,
            _ => {}
        }
    }
}

fn parse_property_array(pair: Pair<Rule>) -> PropertyArray {
    if let Some(pair) = pair.into_inner().next() {
        PropertyArray::Sized(pair.as_str().trim().to_owned())
    } else {
        PropertyArray::Unsized
    }
}

fn parse_element_delegate(
    pair: Pair<Rule>,
    doc_comments: &Option<String>,
    _document: &mut Document,
    filename: &str
) -> Delegate {
    let mut result = Delegate {
        doc_comments: doc_comments.to_owned(),
        ..Default::default()
    };
    result.filename = filename.to_string();
    result.fileline = pair.line_col().0;

    if pair.as_rule() == Rule::element_multicast_delegate || pair.as_rule() == Rule::element_dyn_multicast_delegate {
        result.multicast = true;
    }
    if pair.as_rule() == Rule::element_dynamic_delegate || pair.as_rule() == Rule::element_dyn_multicast_delegate {
        result.dynamic = true;
    }

    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::udelegate => result.specifiers = Some(parse_specifiers(pair)),
            Rule::delegate_name => result.name = pair.as_str().to_owned(),
            Rule::delegate_arguments => parse_delegate_args(pair, &mut result),
            Rule::dynamic_delegate_arguments => parse_delegate_args(pair, &mut result),
            _ => {}
        }
    }

    result
}

fn parse_delegate_args(pair: Pair<Rule>, delegate: &mut Delegate) {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::delegate_argument => {
                    delegate.arguments.push(parse_delegate_arg(pair, delegate.dynamic));
            },
            Rule::dynamic_delegate_argument => {
                delegate.arguments.push(parse_delegate_arg(pair, delegate.dynamic));
            }
            _ => {}
        }
    }
}

fn parse_delegate_arg(pair: Pair<Rule>, is_dynamic: bool) -> Argument {
    let mut arg = Argument::default();
    for pair in pair.into_inner() {
        if is_dynamic {
            match pair.as_rule() {
                Rule::identifier => {
                    arg.name = Some(parse_identifier(pair));
                }
                Rule::value_type => {
                    arg.value_type = parse_value_type(pair);
                }
                _ => {}
            }
        }
        else {
            match pair.as_rule() {
                Rule::delegate_argument_name =>
                {
                    arg.name = Some(parse_identifier(pair));
                }
                Rule::value_type =>
                {
                    arg.value_type = parse_value_type(pair);
                }
                _ => {}
            }
        }
    }
    arg
}

fn parse_element_function(
    pair: Pair<Rule>,
    doc_comments: &Option<String>,
    visibility: Visibility,
    document: &mut Document,
    filename: &str
) -> Function {
    let mut result = Function {
        doc_comments: doc_comments.to_owned(),
        visibility,
        ..Default::default()
    };
    result.filename = filename.to_string();
    result.fileline = pair.line_col().0;

    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::ufunction => result.specifiers = Some(parse_specifiers(pair)),
            Rule::function_signature | Rule::constructor_signature => {
                parse_function_signature(pair, &mut result)
            }
            Rule::function_body => parse_function_body(pair, document),
            _ => {}
        }
    }
    result
}

fn parse_function_signature(pair: Pair<Rule>, result: &mut Function) {
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::template_declaration => result.template = Some(parse_template_declaration(pair)),
            Rule::virtualness => result.is_virtual = true,
            Rule::value_type => result.return_type = Some(parse_value_type(pair)),
            Rule::operator | Rule::identifier => result.name = parse_identifier(pair),
            Rule::function_arguments => parse_function_arguments(pair, result),
            Rule::constness => result.is_const_this = true,
            Rule::overrideness => result.is_override = true,
            Rule::staticness => result.is_static = true,
            _ => {}
        }
    }
}

fn parse_function_arguments(pair: Pair<Rule>, result: &mut Function) {
    for pair in pair.into_inner() {
        if pair.as_rule() == Rule::function_argument {
            result.arguments.push(parse_function_argument(pair));
        }
    }
}

fn parse_function_argument(pair: Pair<Rule>) -> Argument {
    let mut result = Argument::default();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::doc_comment_lines => result.doc_comments = Some(parse_doc_comments(pair)),
            Rule::value_type => result.value_type = parse_value_type(pair),
            Rule::identifier => result.name = Some(parse_identifier(pair)),
            Rule::default_value => result.default_value = Some(parse_default_value(pair)),
            _ => {}
        }
    }
    result
}

fn parse_function_body(pair: Pair<Rule>, document: &mut Document) {
    for pair in pair.into_inner() {
        if pair.as_rule() == Rule::snippet {
            parse_snippet(pair, document);
        }
    }
}

fn parse_default_value(pair: Pair<Rule>) -> String {
    pair.into_inner().next().unwrap().as_str().to_owned()
}

fn parse_value_type(pair: Pair<Rule>) -> String {
    pair.as_str().trim().to_owned()
}

fn parse_template_declaration(pair: Pair<Rule>) -> String {
    pair.as_str().trim().to_owned()
}

fn parse_visibility(pair: Pair<Rule>) -> Option<Visibility> {
    match pair.as_str() {
        "private" => Some(Visibility::Private),
        "protected" => Some(Visibility::Protected),
        "public" => Some(Visibility::Public),
        _ => None,
    }
}

fn parse_inheritances(pair: Pair<Rule>) -> Vec<(Visibility, String)> {
    let mut result = vec![];
    for pair in pair.into_inner() {
        let mut pairs = pair.into_inner();
        result.push((
            parse_visibility(pairs.next().unwrap()).unwrap(),
            parse_value_type(pairs.next().unwrap()),
        ));
    }
    result
}

fn parse_identifier(pair: Pair<Rule>) -> String {
    pair.as_str().to_owned()
}

#[test]
fn test_parsing() {
    let content = crate::read_file("resources/source/test.h").unwrap();
    let mut document = Document::default();
    parse_unreal_cpp_header(&content, &mut document, &Default::default(), Path::new("Test.h"))
        .unwrap_or_else(|error| panic!("Error parsing C++ header: {}", error));
}
