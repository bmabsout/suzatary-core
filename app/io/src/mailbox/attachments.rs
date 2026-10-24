//! A message's attachments, read from the raw maildir file with `mail-parser`, because
//! notmuch's JSON never carries a binary part's content. Pure over captured bytes; what a
//! MIME type's text is belongs to the caller, which may need a subprocess.

use mail_parser::{MessageParser, MimeHeaders as _};
use suzatary_core::mail::AttachmentMeta;

/// Every attachment one raw RFC 5322 message carries, in the order it lists them; `[]` for a
/// message that does not parse.
#[must_use]
pub fn list(bytes: &[u8]) -> Vec<AttachmentMeta> {
    let Some(message) = MessageParser::default().parse(bytes) else {
        return Vec::new();
    };
    message.attachments().map(meta_of).collect()
}

/// One named attachment's MIME type and decoded bytes; the first match when a name repeats.
#[must_use]
pub fn find(bytes: &[u8], name: &str) -> Option<(String, Vec<u8>)> {
    let message = MessageParser::default().parse(bytes)?;
    message
        .attachments()
        .find(|part| attachment_name(part) == name)
        .map(|part| (mime_of(part), part.contents().to_vec()))
}

fn meta_of(part: &mail_parser::MessagePart<'_>) -> AttachmentMeta {
    AttachmentMeta {
        name: attachment_name(part),
        mime: mime_of(part),
        size: part.contents().len(),
    }
}

/// `Content-Disposition`'s `filename`, else `Content-Type`'s `name`, read one way by
/// [`meta_of`] and [`find`].
fn attachment_name(part: &mail_parser::MessagePart<'_>) -> String {
    part.attachment_name().unwrap_or_default().to_owned()
}

/// `type/subtype`, lower-cased; `""` when the part has no `Content-Type`.
pub(crate) fn mime_of(part: &mail_parser::MessagePart<'_>) -> String {
    part.content_type().map_or_else(String::new, |ct| {
        ct.c_subtype.as_deref().map_or_else(
            || ct.c_type.to_lowercase(),
            |sub| format!("{}/{}", ct.c_type.to_lowercase(), sub.to_lowercase()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `multipart/mixed` message with a text and a base64 PDF attachment beside an inline
    /// body, the common shape a mail client sends.
    fn message_with_attachments() -> Vec<u8> {
        let pdf = crate::pdftotext::minimal_pdf("Salary USD 95000");
        let pdf_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &pdf);
        format!(
            "From: hr@example.test\r\n\
             To: ada@example.test\r\n\
             Subject: Offer letter\r\n\
             MIME-Version: 1.0\r\n\
             Content-Type: multipart/mixed; boundary=\"BOUNDARY\"\r\n\
             \r\n\
             --BOUNDARY\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\
             \r\n\
             Please see the attached offer.\r\n\
             --BOUNDARY\r\n\
             Content-Type: text/plain; charset=utf-8; name=\"notes.txt\"\r\n\
             Content-Disposition: attachment; filename=\"notes.txt\"\r\n\
             \r\n\
             Start date: 1 October\r\n\
             --BOUNDARY\r\n\
             Content-Type: application/pdf; name=\"offer.pdf\"\r\n\
             Content-Disposition: attachment; filename=\"offer.pdf\"\r\n\
             Content-Transfer-Encoding: base64\r\n\
             \r\n\
             {pdf_b64}\r\n\
             --BOUNDARY--\r\n"
        )
        .into_bytes()
    }

    /// Two attachments, not three: the first part, with no disposition, is the inline body.
    #[test]
    fn list_names_every_attachment_with_its_mime_and_size() {
        let listed = list(&message_with_attachments());
        assert_eq!(listed.len(), 2, "{listed:?}");
        let notes = listed
            .iter()
            .find(|one| one.name == "notes.txt")
            .expect("notes.txt");
        assert_eq!(notes.mime, "text/plain");
        assert_eq!(notes.size, b"Start date: 1 October".len());
        let offer = listed
            .iter()
            .find(|one| one.name == "offer.pdf")
            .expect("offer.pdf");
        assert_eq!(offer.mime, "application/pdf");
        assert!(offer.size > 0);
    }

    #[test]
    fn find_reads_the_named_attachments_decoded_bytes() {
        let bytes = message_with_attachments();
        let (mime, content) = find(&bytes, "notes.txt").expect("notes.txt");
        assert_eq!(mime, "text/plain");
        assert_eq!(content, b"Start date: 1 October");

        let (mime, content) = find(&bytes, "offer.pdf").expect("offer.pdf");
        assert_eq!(mime, "application/pdf");
        // Decoded back to the exact bytes, not the base64 wire form.
        assert_eq!(content, crate::pdftotext::minimal_pdf("Salary USD 95000"));
    }

    #[test]
    fn find_answers_none_for_a_name_this_message_does_not_carry() {
        assert_eq!(find(&message_with_attachments(), "nope.pdf"), None);
    }

    #[test]
    fn list_and_find_answer_nothing_for_bytes_mail_parser_cannot_parse() {
        assert_eq!(list(b""), Vec::new());
        assert_eq!(find(b"", "anything"), None);
    }
}
