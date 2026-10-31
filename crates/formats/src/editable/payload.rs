use scribblecap_core::{Document, ResourceId};

use super::{
    EditableDocument, EditableError, MAX_ID_BYTES, MAX_JSON_BYTES, MAX_PAYLOAD_BYTES,
    MAX_RESOURCES, ResourceData, validate_resources,
};

pub(super) const MAGIC: &[u8; 8] = b"SCAPDOC\0";

pub(super) fn encode(
    editable: &EditableDocument,
    fingerprint: u32,
) -> Result<Vec<u8>, EditableError> {
    validate_resources(&editable.document, &editable.resources)?;
    let json = editable
        .document
        .to_json()
        .map_err(EditableError::Validation)?;
    if json.len() > MAX_JSON_BYTES {
        return Err(EditableError::Limits);
    }
    let resources: std::collections::HashMap<_, _> = editable
        .resources
        .iter()
        .map(|r| (&r.id, r.bytes.as_slice()))
        .collect();
    let mut length = 32usize
        .checked_add(json.len())
        .ok_or(EditableError::Limits)?;
    for resource in &editable.document.resources {
        length = length
            .checked_add(12)
            .and_then(|v| v.checked_add(resource.id.0.len()))
            .and_then(|v| v.checked_add(resources[&resource.id].len()))
            .ok_or(EditableError::Limits)?;
    }
    if length > MAX_PAYLOAD_BYTES {
        return Err(EditableError::Limits);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(length)
        .map_err(|_| EditableError::Limits)?;
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&fingerprint.to_be_bytes());
    out.extend_from_slice(&(json.len() as u64).to_be_bytes());
    out.extend_from_slice(&(resources.len() as u32).to_be_bytes());
    out.extend_from_slice(json.as_bytes());
    for resource in &editable.document.resources {
        let bytes = resources[&resource.id];
        out.extend_from_slice(&(resource.id.0.len() as u32).to_be_bytes());
        out.extend_from_slice(resource.id.0.as_bytes());
        out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        out.extend_from_slice(bytes);
    }
    out.extend_from_slice(&crc32fast::hash(&out).to_be_bytes());
    Ok(out)
}

pub(super) fn decode(bytes: &[u8]) -> Result<(EditableDocument, u32), EditableError> {
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(EditableError::Limits);
    }
    if bytes.len() < 32 {
        return Err(EditableError::InvalidPayload("truncated header"));
    }
    let split = bytes.len() - 4;
    let mut checksum = Reader(&bytes[split..]);
    if crc32fast::hash(&bytes[..split]) != u32::from_be_bytes(checksum.array()?) {
        return Err(EditableError::InvalidPayload("checksum mismatch"));
    }
    let mut reader = Reader(&bytes[..split]);
    if reader.take(8)? != MAGIC {
        return Err(EditableError::InvalidPayload("wrong magic"));
    }
    let version = u32::from_be_bytes(reader.array()?);
    if version != 1 {
        return Err(EditableError::UnsupportedVersion(version));
    }
    let fingerprint = u32::from_be_bytes(reader.array()?);
    let json_length =
        usize::try_from(u64::from_be_bytes(reader.array()?)).map_err(|_| EditableError::Limits)?;
    let count = u32::from_be_bytes(reader.array()?) as usize;
    if json_length > MAX_JSON_BYTES || count > MAX_RESOURCES {
        return Err(EditableError::Limits);
    }
    let json = std::str::from_utf8(reader.take(json_length)?)
        .map_err(|_| EditableError::InvalidPayload("document is not UTF-8"))?;
    let document = Document::from_json(json).map_err(EditableError::Document)?;
    if count != document.resources.len() {
        return Err(EditableError::InvalidResources);
    }
    let mut resources = Vec::with_capacity(count);
    for _ in 0..count {
        let id_length = u32::from_be_bytes(reader.array()?) as usize;
        if id_length == 0 || id_length > MAX_ID_BYTES {
            return Err(EditableError::Limits);
        }
        let id = std::str::from_utf8(reader.take(id_length)?)
            .map_err(|_| EditableError::InvalidPayload("resource id is not UTF-8"))?;
        let length = usize::try_from(u64::from_be_bytes(reader.array()?))
            .map_err(|_| EditableError::Limits)?;
        let data = reader.take(length)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(data.len())
            .map_err(|_| EditableError::Limits)?;
        bytes.extend_from_slice(data);
        resources.push(ResourceData {
            id: ResourceId(id.to_owned()),
            bytes,
        });
    }
    if !reader.0.is_empty() {
        return Err(EditableError::InvalidPayload("unexpected trailing bytes"));
    }
    Ok((EditableDocument::new(document, resources)?, fingerprint))
}

struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], EditableError> {
        let bytes = self.0.get(..length).ok_or(EditableError::InvalidPayload(
            "truncated data or forged length",
        ))?;
        self.0 = &self.0[length..];
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], EditableError> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scribblecap_core::Size;

    fn minimal() -> Vec<u8> {
        encode(
            &EditableDocument::new(Document::new(Size::new(1.0, 1.0)), vec![]).unwrap(),
            123,
        )
        .unwrap()
    }

    fn reseal(bytes: &mut [u8]) {
        let end = bytes.len() - 4;
        let crc = crc32fast::hash(&bytes[..end]);
        bytes[end..].copy_from_slice(&crc.to_be_bytes());
    }

    #[test]
    fn future_versions_and_invalid_lengths_are_rejected() {
        let mut bytes = minimal();
        bytes[8..12].copy_from_slice(&2u32.to_be_bytes());
        reseal(&mut bytes);
        assert!(matches!(
            decode(&bytes),
            Err(EditableError::UnsupportedVersion(2))
        ));
        let mut bytes = minimal();
        bytes[16..24].copy_from_slice(&u64::MAX.to_be_bytes());
        reseal(&mut bytes);
        assert!(matches!(decode(&bytes), Err(EditableError::Limits)));
        let mut bytes = minimal();
        bytes[24..28].copy_from_slice(&u32::MAX.to_be_bytes());
        reseal(&mut bytes);
        assert!(matches!(decode(&bytes), Err(EditableError::Limits)));
    }

    #[test]
    fn checksums_json_and_trailing_data_are_validated() {
        let mut bytes = minimal();
        bytes[28] ^= 1;
        assert!(matches!(
            decode(&bytes),
            Err(EditableError::InvalidPayload(_))
        ));
        reseal(&mut bytes);
        assert!(matches!(decode(&bytes), Err(EditableError::Document(_))));
        let mut bytes = minimal();
        let at = bytes.len() - 4;
        bytes.insert(at, 0);
        reseal(&mut bytes);
        assert!(matches!(
            decode(&bytes),
            Err(EditableError::InvalidPayload(_))
        ));
    }

    #[test]
    fn all_truncated_payload_prefixes_fail() {
        let bytes = minimal();
        for at in 0..bytes.len() {
            assert!(decode(&bytes[..at]).is_err(), "prefix {at}");
        }
    }

    #[test]
    fn forged_resource_lengths_and_invalid_ids_are_rejected() {
        let mut document = Document::new(Size::new(1.0, 1.0));
        let id = ResourceId::from("bg");
        document.resources.push(scribblecap_core::Resource {
            id: id.clone(),
            media_type: "image/png".into(),
        });
        let bundle = EditableDocument::new(
            document,
            vec![ResourceData {
                id,
                bytes: vec![1, 2, 3],
            }],
        )
        .unwrap();
        let valid = encode(&bundle, 0).unwrap();
        let json_length = u64::from_be_bytes(valid[16..24].try_into().unwrap()) as usize;
        let entry = 28 + json_length;
        let mut bytes = valid.clone();
        bytes[entry..entry + 4].copy_from_slice(&u32::MAX.to_be_bytes());
        reseal(&mut bytes);
        assert!(matches!(decode(&bytes), Err(EditableError::Limits)));
        let mut bytes = valid.clone();
        bytes[entry + 4] = 0xff;
        reseal(&mut bytes);
        assert!(matches!(
            decode(&bytes),
            Err(EditableError::InvalidPayload(_))
        ));
        let mut bytes = valid;
        bytes[entry + 6..entry + 14].copy_from_slice(&u64::MAX.to_be_bytes());
        reseal(&mut bytes);
        assert!(decode(&bytes).is_err());
    }
}
