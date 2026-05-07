use crate::{
    HasEntries, parse_as_syntax_resource, resource_messages_mut, serialize_resources_to_files,
};
use anyhow::{Context, Result, bail};
use fluent_syntax::ast::{Comment, Message, Resource};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    path::Path,
};

/// Annotation on a message, specified via a message comment line starting with [`Annotation::PREFIX`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Annotation {
    /// Set by tooling when the message definition in the source code changes.
    /// This indicates that the developer who made the change needs to address this.
    /// While this annotation is present, checks should fail to indicate that they need to be
    /// resolved.
    Outdated,
    /// This indicates that the message can be used in its current state, but might benefit from
    /// attention by a translator.
    NeedsReview,
}

impl Annotation {
    const PREFIX: &str = "ANNOTATION: ";
    const VALUE_OUTDATED: &str = "OUTDATED";
    const VALUE_NEEDS_REVIEW: &str = "NEEDS-REVIEW";
}

impl TryFrom<&str> for Annotation {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            Self::VALUE_OUTDATED => Ok(Self::Outdated),
            Self::VALUE_NEEDS_REVIEW => Ok(Self::NeedsReview),
            _ => bail!("Unexpected annotation: {value}"),
        }
    }
}

impl AsRef<str> for Annotation {
    fn as_ref(&self) -> &'static str {
        match *self {
            Self::Outdated => Self::VALUE_OUTDATED,
            Self::NeedsReview => Self::VALUE_NEEDS_REVIEW,
        }
    }
}

pub trait MessageAnnotationRead {
    fn get_all_annotations(&self) -> Result<HashSet<Annotation>>;
    fn has_annotation(&self, annotation: Annotation) -> Result<bool>;
}

pub trait MessageAnnotationWrite {
    fn add_annotation(&mut self, annotation: Annotation) -> Result<()>;
    fn remove_annotation(&mut self, annotation: Annotation) -> Result<bool>;
    fn format(&mut self) -> Result<()>;
}

impl<S: AsRef<str>> MessageAnnotationRead for Message<S> {
    fn get_all_annotations(&self) -> Result<HashSet<Annotation>> {
        let mut annotations = HashSet::new();
        let Some(comment) = &self.comment else {
            return Ok(annotations);
        };
        for line in &comment.content {
            if let Some(annotation_str) = line.as_ref().strip_prefix(Annotation::PREFIX) {
                annotations.insert(Annotation::try_from(annotation_str).with_context(|| {
                    format!("Annotation error in message {}", &self.id.name.as_ref())
                })?);
            }
        }
        Ok(annotations)
    }

    fn has_annotation(&self, annotation: Annotation) -> Result<bool> {
        let Some(comment) = &self.comment else {
            return Ok(false);
        };
        for line in &comment.content {
            if let Some(annotation_str) = line.as_ref().strip_prefix(Annotation::PREFIX) {
                if Annotation::try_from(annotation_str).with_context(|| {
                    format!("Annotation error in message {}", &self.id.name.as_ref())
                })? == annotation
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl MessageAnnotationWrite for Message<String> {
    fn add_annotation(&mut self, annotation: Annotation) -> Result<()> {
        let mut annotations = remove_all_annotations(self)?;
        annotations.insert(annotation);
        let mut new_comment_lines = annotations_to_comment_lines(&annotations);
        if let Some(comment) = self.comment.take() {
            for line in comment.content {
                new_comment_lines.push(line);
            }
        }
        self.comment = Some(Comment {
            content: new_comment_lines,
        });
        Ok(())
    }

    fn remove_annotation(&mut self, annotation: Annotation) -> Result<bool> {
        let mut annotations = remove_all_annotations(self)?;
        let was_present = annotations.remove(&annotation);
        let mut new_comment_lines = annotations_to_comment_lines(&annotations);
        if let Some(comment) = self.comment.take() {
            for line in comment.content {
                new_comment_lines.push(line);
            }
        }
        self.comment = Some(Comment {
            content: new_comment_lines,
        });
        Ok(was_present)
    }

    fn format(&mut self) -> Result<()> {
        let annotations = remove_all_annotations(self)?;
        let mut new_comment_lines = annotations_to_comment_lines(&annotations);
        if let Some(comment) = self.comment.take() {
            for line in comment.content {
                new_comment_lines.push(line);
            }
        }
        self.comment = Some(Comment {
            content: new_comment_lines,
        });
        Ok(())
    }
}

fn remove_all_annotations<S: AsRef<str> + Clone>(
    message: &mut Message<S>,
) -> Result<HashSet<Annotation>> {
    let mut annotations = HashSet::new();
    let Some(comment) = &mut message.comment else {
        return Ok(annotations);
    };
    let mut updated_comment = vec![];
    for line in &comment.content {
        if let Some(annotation_str) = line.as_ref().strip_prefix(Annotation::PREFIX) {
            let annotation = Annotation::try_from(annotation_str).with_context(|| {
                format!("Annotation error in message {}", message.id.name.as_ref())
            })?;
            annotations.insert(annotation);
        } else {
            updated_comment.push(line.to_owned());
        }
    }
    comment.content = updated_comment;
    Ok(annotations)
}

fn annotations_to_comment_lines(annotations: &HashSet<Annotation>) -> Vec<String> {
    let mut annotations = annotations.iter().collect::<Vec<_>>();
    annotations.sort();
    annotations
        .into_iter()
        .map(|a| {
            let mut annotation_line = Annotation::PREFIX.to_owned();
            annotation_line.push_str(a.as_ref());
            annotation_line
        })
        .collect::<Vec<_>>()
}

pub fn get_message_ids_with_different_value<'a, S: AsRef<str> + PartialEq>(
    old: &'a Resource<S>,
    new: &'a Resource<S>,
) -> HashSet<&'a str> {
    let old_messages = old.messages();
    let new_messages = new.messages();

    let mut diff = HashSet::new();
    for (id, old_message) in old_messages {
        if let Some(new_message) = new_messages.get(id) {
            if old_message.value != new_message.value
                || old_message.attributes != new_message.attributes
            {
                diff.insert(id);
            }
        }
    }
    diff
}

pub fn modify_messages<P: AsRef<Path> + Eq + Hash, S: AsRef<str>, T>(
    modify: impl Fn(&mut Message<String>) -> Result<T>,
    message_ids: &HashSet<S>,
    resource_files: &[P],
) -> Result<()> {
    let mut resources = resource_files
        .iter()
        .map(parse_as_syntax_resource)
        .collect::<Result<Vec<_>>>()?;
    for res in &mut resources {
        let mut messages = resource_messages_mut(res);
        for message_id in message_ids {
            if let Some(message) = messages.get_mut(message_id.as_ref()) {
                modify(message)?;
            }
        }
    }
    let resource_map: HashMap<&P, Resource<String>> =
        HashMap::from_iter(resource_files.iter().zip(resources));
    serialize_resources_to_files(&resource_map)
}

pub fn add_annotation_to_messages<P: AsRef<Path> + Eq + Hash, S: AsRef<str>>(
    annotation: Annotation,
    message_ids: &HashSet<S>,
    resource_files: &[P],
) -> Result<()> {
    modify_messages(
        |message| message.add_annotation(annotation),
        message_ids,
        resource_files,
    )
}

pub fn remove_annotation_from_messages<P: AsRef<Path> + Eq + Hash, S: AsRef<str>>(
    annotation: Annotation,
    message_ids: &HashSet<S>,
    resource_files: &[P],
) -> Result<()> {
    modify_messages(
        |message| message.remove_annotation(annotation),
        message_ids,
        resource_files,
    )
}
