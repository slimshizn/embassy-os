use std::io::{Read, Seek, SeekFrom, Write};

use typed_builder::TypedBuilder;

use super::header::{FileSection, Header};
use super::manifest::Manifest;
use crate::config::ConfigSpec;
use crate::{Error, ResultExt};

#[derive(TypedBuilder)]
pub struct S9pkPacker<'a, W: Write + Seek, RIcon: Read, RAppImage: Read> {
    writer: W,
    manifest: &'a Manifest,
    config_spec: &'a ConfigSpec,
    icon: RIcon,
    app_image: RAppImage,
    #[builder(default)]
    instructions: Option<&'a str>,
}
impl<'a, W: Write + Seek, RIcon: Read, RAppImage: Read> S9pkPacker<'a, W, RIcon, RAppImage> {
    /// BLOCKING
    pub fn pack(mut self) -> Result<(), Error> {
        let header_pos = self.writer.stream_position()?;
        if header_pos != 0 {
            log::warn!("Appending to non-empty file.");
        }
        let mut header = Header::placeholder();
        header.serialize(&mut self.writer).with_ctx(|_| {
            (
                crate::ErrorKind::Serialization,
                "Writing Placeholder Header",
            )
        })?;
        let mut position = self.writer.stream_position()?;
        // manifest
        serde_cbor::to_writer(&mut self.writer, self.manifest).with_ctx(|_| {
            (
                crate::ErrorKind::Serialization,
                "Serializing Manifest (CBOR)",
            )
        })?;
        let new_pos = self.writer.stream_position()?;
        header.table_of_contents.manifest = FileSection {
            position,
            length: new_pos - position,
        };
        position = new_pos;
        // config_spec
        serde_cbor::to_writer(&mut self.writer, self.config_spec).with_ctx(|_| {
            (
                crate::ErrorKind::Serialization,
                "Serializing Config Spec (CBOR)",
            )
        })?;
        let new_pos = self.writer.stream_position()?;
        header.table_of_contents.config_spec = FileSection {
            position,
            length: new_pos - position,
        };
        position = new_pos;
        // icon
        std::io::copy(&mut self.icon, &mut self.writer)
            .with_ctx(|_| (crate::ErrorKind::Filesystem, "Copying Icon"))?;
        let new_pos = self.writer.stream_position()?;
        header.table_of_contents.icon = FileSection {
            position,
            length: new_pos - position,
        };
        position = new_pos;
        // app_image
        std::io::copy(&mut self.app_image, &mut self.writer)
            .with_ctx(|_| (crate::ErrorKind::Filesystem, "Copying App Image"))?;
        let new_pos = self.writer.stream_position()?;
        header.table_of_contents.app_image = FileSection {
            position,
            length: new_pos - position,
        };
        position = new_pos;
        // instructions
        if let Some(instructions) = self.instructions {
            self.writer
                .write(instructions.as_bytes())
                .with_ctx(|_| (crate::ErrorKind::Filesystem, "Packing App Image"))?;
            let new_pos = self.writer.stream_position()?;
            header.table_of_contents.app_image = FileSection {
                position,
                length: new_pos - position,
            };
            position = new_pos;
        }
        // header
        self.writer.seek(SeekFrom::Start(header_pos))?;
        header
            .serialize(&mut self.writer)
            .with_ctx(|_| (crate::ErrorKind::Serialization, "Writing Header"))?;
        self.writer.seek(SeekFrom::Start(position))?;

        Ok(())
    }
}
