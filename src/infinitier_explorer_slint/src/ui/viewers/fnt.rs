//! FNT viewer. Mirrors the NearInfinity-style struct table the egui
//! viewer shows, plus a short note + a hex preview of the opaque
//! post-header body bytes.

use bytesize::ByteSize;
use infinitier_core::game::GameResource;
use infinitier_core::resource::fnt::{Fnt, HEADER_LEN};

use crate::{KeyValue, MainWindow};
use crate::ui::viewers::common;

pub fn populate(window: &MainWindow, fnt: Fnt, resource: &GameResource) {
    let body_len = fnt.body().len();

    let rows = vec![
        KeyValue {
            label: "# extra letters".into(),
            value: fnt.extra_letters_count.to_string().into(),
        },
        KeyValue {
            label: "Header size".into(),
            value: HEADER_LEN.to_string().into(),
        },
        KeyValue {
            label: "Letters".into(),
            value: fnt.letters_bam.clone().into(),
        },
        KeyValue {
            label: "Extra letters".into(),
            value: fnt.extra_letters_bmp.clone().into(),
        },
    ];

    let note = if body_len == 0 {
        String::new()
    } else {
        format!(
            "Note: FNT is a stub. Glyph data lives in {} and {}; the {body_len} bytes past offset 0x04 in this file are engine-internal and not parsed (NearInfinity treats them the same way).",
            fnt.letters_bam, fnt.extra_letters_bmp,
        )
    };

    let raw_dump = if body_len == 0 {
        String::new()
    } else {
        const PREVIEW_BYTES: usize = 256;
        let body = fnt.body();
        let shown = body.len().min(PREVIEW_BYTES);
        let mut dump = String::with_capacity(shown * 4);
        dump.push_str(&format!(
            "Showing first {shown} of {} bytes (offset 0x{HEADER_LEN:X} in file).\n\n",
            body.len(),
        ));
        for (i, chunk) in body[..shown].chunks(16).enumerate() {
            dump.push_str(&format!("{:08x}  ", HEADER_LEN + i * 16));
            for b in chunk {
                dump.push_str(&format!("{:02x} ", b));
            }
            for _ in chunk.len()..16 {
                dump.push_str("   ");
            }
            dump.push(' ');
            for &b in chunk {
                dump.push(if (32..127).contains(&b) {
                    b as char
                } else {
                    '.'
                });
            }
            dump.push('\n');
        }
        dump
    };

    window.set_viewer_kind("fnt".into());
    window.set_fnt_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
    window.set_fnt_note(note.into());
    window.set_fnt_raw_dump(raw_dump.into());
    window.set_fnt_file_size(common::file_size_text(resource).into());
    window.set_fnt_body_size(format!("body: {} (opaque)", ByteSize(body_len as u64)).into());
    window.set_fnt_origin(common::origin_text(resource).into());
}
