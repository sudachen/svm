use std::u16;

use indexmap::IndexMap;

use svm_types::{Section, SectionKind, Sections};

use crate::WriteExt;

use super::preview;
use super::SectionPreview;

/// A trait to be implemented by [`Section`] encoders.
pub trait SectionEncoder {
    /// Encodes `Self` into its binary format. Bytes are appended into `w`.
    fn encode(&self, w: &mut Vec<u8>);
}

/// Encodes a collection of [`Section`] into a binary form.
pub struct SectionsEncoder {
    section_buf: IndexMap<SectionKind, Vec<u8>>,
}

impl Default for SectionsEncoder {
    fn default() -> Self {
        Self {
            section_buf: IndexMap::with_capacity(0),
        }
    }
}

impl SectionsEncoder {
    /// Creates a new encoder,and allocates room for `capacity` sections.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            section_buf: IndexMap::with_capacity(capacity),
        }
    }

    /// Encodes each [`Section`] provided by `sections` and stores them internally.
    pub fn encode(&mut self, sections: &Sections) {
        for section in sections.iter() {
            self.encode_section(section);
        }
    }

    /// Returns the binary encoding of the [`Section`]s provided so far.
    pub fn finish(mut self) -> Vec<u8> {
        let section_count = self.section_buf.len();

        assert!(section_count < std::u16::MAX as usize);

        let section_count_size = 2;
        let previews_size = section_count * SectionPreview::len();
        let sections_size: usize = self.section_buf.values().map(|buf| buf.len()).sum();

        let capacity = section_count_size + previews_size + sections_size;

        let mut w = Vec::with_capacity(capacity);

        // Section Count
        w.write_u16_be(section_count as u16);

        for (kind, bytes) in self.section_buf.drain(..) {
            // Section Preview
            let byte_size = bytes.len();

            assert!(byte_size < std::u32::MAX as usize);

            let preview = SectionPreview::new(kind, byte_size as u32);
            preview::encode(&preview, &mut w);

            // `Section`
            w.write_bytes(&bytes);
        }

        w
    }

    fn encode_section(&mut self, section: &Section) {
        let kind = section.kind();
        let buf = self.section_buf_mut(kind);

        let encoder: &dyn SectionEncoder = match kind {
            SectionKind::Api => section.as_api(),
            SectionKind::Header => section.as_header(),
            SectionKind::Code => section.as_code(),
            SectionKind::Data => section.as_data(),
            SectionKind::Ctors => section.as_ctors(),
            SectionKind::Schema => section.as_schema(),
            SectionKind::Deploy => section.as_deploy(),
        };

        encoder.encode(buf);
    }

    fn section_buf_mut(&mut self, kind: SectionKind) -> &mut Vec<u8> {
        // initializes an `Section buffer` if not exists
        let _entry = self.section_buf.entry(kind).or_insert_with(|| Vec::new());

        if let Some(buf) = self.section_buf.get_mut(&kind) {
            buf
        } else {
            unreachable!()
        }
    }
}
