use xml::{
    attribute::OwnedAttribute, name::OwnedName, reader::XmlEvent, EventReader,
};

struct DocContext {
    stack: Vec<DocType>,
    section: DocSection,
}

#[derive(Debug, PartialEq)]
enum DocSection {
    None,
    Parameters,
    Returns,
    Remarks,
}

impl DocContext {
    fn open_element(&mut self, doc: DocType) {
        match doc {
            DocType::Parameter { .. } => {
                self.push_section_maybe(DocSection::Parameters)
            }
            DocType::Returns(_) => self.push_section_maybe(DocSection::Returns),
            DocType::Remarks(_) => self.push_section_maybe(DocSection::Remarks),
            _ => {}
        };
        self.stack.push(doc);
    }

    fn close_element(&mut self) -> Option<String> {
        let end = self.stack.pop().expect("context already closed");
        if let Some(parent) = self.stack.last_mut() {
            parent.push_str(&end.to_string());
        }

        // Consume pseudo elements.
        while let Some(DocType::Section(_)) = self.stack.last() {
            self.close_element();
        }

        // If we've popped back to the root, we're done. Shovel it.
        if self.stack.is_empty() {
            Some(end.to_string())
        } else {
            None
        }
    }

    fn push_str(&mut self, str: &str) {
        self.stack
            .last_mut()
            .expect("writing to an empty context")
            .push_str(str);
    }

    fn push_section_maybe(&mut self, section: DocSection) {
        if section != self.section {
            self.stack
                .push(DocType::Section(format!("# {:?}\n", section)));
            self.section = section;
        }
    }
}

enum DocType {
    Code(String),
    Doc(String),
    Link { href: String, label: String },
    Math(String),
    ParamRef(String),
    Parameter { name: String, label: String },
    Remarks(String),
    Returns(String),
    Section(String),
    See { cref: String },
    Summary(String),
}

impl DocType {
    fn from_event(name: OwnedName, attrs: Vec<OwnedAttribute>) -> Self {
        match name.to_string().as_ref() {
            "summary" => Self::Summary(String::new()),
            "a" => Self::Link {
                href: Self::find_attr("href", attrs)
                    .expect("a tag with no href"),
                label: String::new(),
            },
            "c" => Self::Code(String::new()),
            "doc" => Self::Doc(String::new()),
            "param" => Self::Parameter {
                name: Self::find_attr("name", attrs)
                    .expect("param tag with no name"),
                label: String::new(),
            },
            "paramref" => Self::ParamRef(
                Self::find_attr("name", attrs)
                    .expect("paramref tag with no name"),
            ),
            "returns" => Self::Returns(String::new()),
            "remarks" => Self::Remarks(String::new()),
            "see" => Self::See {
                cref: Self::find_attr("cref", attrs).expect("see with no cref"),
            },
            "math" => Self::Math(String::new()),
            _ => panic!("Unrecognized doc element: {}", name),
        }
    }

    fn push_str(&mut self, str: &str) {
        match self {
            Self::Summary(s)
            | Self::Doc(s)
            | Self::Returns(s)
            | Self::ParamRef(s)
            | Self::Remarks(s)
            | Self::Code(s)
            | Self::Math(s)
            | Self::Section(s) => s.push_str(str),
            Self::Link { label, .. } | Self::Parameter { label, .. } => {
                label.push_str(str)
            }
            Self::See { cref } => cref.push_str(str),
        };
    }

    fn find_attr(key: &str, attrs: Vec<OwnedAttribute>) -> Option<String> {
        attrs
            .iter()
            .find(|attr| attr.name.to_string() == key)
            .map(|attr| attr.value.to_string())
    }
}

impl ToString for DocType {
    fn to_string(&self) -> String {
        match self {
            Self::Summary(s)
            | Self::Doc(s)
            | Self::Returns(s)
            | Self::Remarks(s)
            // TODO: Format math.
            | Self::Math(s)
            | Self::Section(s) => s.to_owned(),
            Self::Link { href, label } => format!("[{label}]({href})"),
            Self::Parameter { name, label } => format!(" - `{name}`: {label}"),
            Self::ParamRef(s) | Self::Code(s) => format!("`{s}`"),
            // TODO: Actually reference the generated procedure definition.
            Self::See { cref } => format!("`{}`", cref.replace("M:", "")),
        }
    }
}

pub fn parse_doc(xml: &str) -> String {
    let parser = EventReader::new(xml.as_bytes());

    let mut ctx = DocContext {
        stack: Vec::new(),
        section: DocSection::None,
    };

    for event in parser {
        match event {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => ctx.open_element(DocType::from_event(name, attributes)),
            Ok(XmlEvent::Whitespace(s)) | Ok(XmlEvent::Characters(s)) => {
                ctx.push_str(&s);
            }
            Ok(XmlEvent::EndElement { .. }) => {
                if let Some(done) = ctx.close_element() {
                    return done;
                }
            }
            _ => continue,
        };
    }

    unreachable!();
}

#[cfg(test)]
mod tests {
    use crate::doc::parse_doc;

    #[test]
    fn test_parse_doc() {
        let sample = "<doc>\n<summary>\nThis service provides functionality to interact with\n<a href=\"https://forum.kerbalspaceprogram.com/index.php?/topic/184787-infernal-robotics-next/\">Infernal Robotics</a>.\n</summary>\n</doc>";
        dbg!(parse_doc(sample));
    }

    #[test]
    fn test_parse_params() {
        let sample = "<doc>\n<summary>\nConstruct a tuple.\n</summary>\n<returns>The tuple.</returns>\n<param name=\"elements\">The elements.</param>\n</doc>";
        dbg!(parse_doc(sample));
    }

    #[test]
    fn test_parse_paramref_code() {
        let sample = "<doc>\n<summary>\nReturns the servo group in the given <paramref name=\"vessel\" /> with the given <paramref name=\"name\" />,\nor <c>null</c> if none exists. If multiple servo groups have the same name, only one of them is returned.\n</summary>\n<param name=\"vessel\">Vessel to check.</param>\n<param name=\"name\">Name of servo group to find.</param>\n</doc>";
        dbg!(parse_doc(sample));
    }
}
