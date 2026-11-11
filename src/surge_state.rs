use quick_xml::{events::Event, Reader};
use serde_json::{json, Value};
use std::collections::BTreeMap;

// The installed Surge state is a sub3 chunk, not an FXP container. Validate the
// bounded XML region before examining actual patch metadata. No guessed preset.
pub fn verify(bytes: &[u8]) -> Result<Value, String> {
    let (name, values) = parse_state(bytes)?;
    if name != "Init Saw" {
        return Err(crate::i18n::t("m083").into());
    }
    let path = "C:/ProgramData/Surge XT/patches_factory/Templates/Init Saw.fxp";
    let factory =
        std::fs::read(path).map_err(|e| crate::i18n::render("f028", &[("e", format!("{}", e))]))?;
    if factory.len() < 60
        || &factory[..4] != b"CcnK"
        || &factory[8..12] != b"FPCh"
        || &factory[16..20] != b"cjs3"
        || u32::from_be_bytes(factory[56..60].try_into().unwrap()) as usize != factory.len() - 60
    {
        return Err(crate::i18n::t("m084").into());
    }
    let (factory_name, expected) = parse_state(&factory[60..])?;
    compare(&name, &values, &factory_name, &expected)?;
    Ok(
        json!({"patchName":name,"metadata":"actual CLAP state and factory FXP XML","factoryReference":path,"parameterCount":values.len(),"matchingFactoryValues":expected.len(),"numericTolerance":1e-6}),
    )
}

fn compare(
    name: &str,
    values: &BTreeMap<String, f64>,
    factory_name: &str,
    expected: &BTreeMap<String, f64>,
) -> Result<(), String> {
    if name != "Init Saw"
        || factory_name != "Init Saw"
        || values.len() != expected.len()
        || expected.is_empty()
    {
        return Err(crate::i18n::t("m085").into());
    }
    for (parameter, expected) in expected {
        let actual = values
            .get(parameter)
            .ok_or_else(|| crate::i18n::render("f029", &[("parameter", parameter.to_string())]))?;
        if (actual - expected).abs() > 1e-6 * expected.abs().max(1.0) {
            return Err(crate::i18n::render(
                "f030",
                &[
                    ("parameter", parameter.to_string()),
                    ("actual", format!("{}", actual)),
                    ("expected", format!("{}", expected)),
                ],
            ));
        }
    }
    Ok(())
}

fn parse_state(bytes: &[u8]) -> Result<(String, BTreeMap<String, f64>), String> {
    if bytes.len() < 32 || bytes.get(..4) != Some(b"sub3") {
        return Err(crate::i18n::t("m086").into());
    }
    let xml_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let xml = bytes
        .get(32..32usize.checked_add(xml_len).ok_or(crate::i18n::t("m087"))?)
        .ok_or(crate::i18n::t("m088"))?;
    let xml = std::str::from_utf8(xml).map_err(|e| e.to_string())?;
    let mut reader = Reader::from_str(xml.trim_end_matches('\0'));
    reader.config_mut().expand_empty_elements = true;
    let mut path = Vec::<String>::new();
    let mut name = String::new();
    let mut values = BTreeMap::new();
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(element) => {
                let tag = element.name().as_ref().to_owned();
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|e| e.to_string())?;
                    let value = attribute
                        .normalized_value(Default::default())
                        .map_err(|e| e.to_string())?;
                    if tag == "meta" && path == ["patch"] && attribute.key.as_ref() == "name" {
                        name = value.into_owned();
                    } else if path == ["patch", "parameters"] && attribute.key.as_ref() == "value" {
                        let number = value.parse::<f64>().map_err(|e| e.to_string())?;
                        if !number.is_finite() || values.insert(tag.clone(), number).is_some() {
                            return Err(crate::i18n::t("m089").into());
                        }
                    }
                }
                path.push(tag);
            }
            Event::End(_) => {
                path.pop();
            }
            Event::Eof => break,
            _ => (),
        }
    }
    if !path.is_empty() || name.is_empty() || values.is_empty() {
        return Err(crate::i18n::t("incomplete_xml").into());
    }
    Ok((name, values))
}

#[cfg(test)]
mod tests {
    use super::{compare, verify};
    use std::collections::BTreeMap;

    #[test]
    fn unknown_or_truncated_state_cannot_be_ready() {
        assert!(verify(b"CcnK-not-a-clap-state").is_err());
        let mut bytes = vec![0; 32];
        bytes[..4].copy_from_slice(b"sub3");
        bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(verify(&bytes).is_err());
    }

    #[test]
    fn other_patch_is_not_claimed_as_init_saw() {
        let xml = b"<patch><meta name=\"Different Patch\" /></patch>";
        let mut bytes = vec![0; 32];
        bytes[..4].copy_from_slice(b"sub3");
        bytes[4..8].copy_from_slice(&(xml.len() as u32).to_le_bytes());
        bytes.extend_from_slice(xml);
        assert!(verify(&bytes).is_err());
    }

    #[test]
    fn retained_patch_name_cannot_hide_changed_sound_parameters() {
        let actual = BTreeMap::from([("a_osc1_type".into(), 1.0)]);
        let factory = BTreeMap::from([("a_osc1_type".into(), 0.0)]);
        assert!(compare("Init Saw", &actual, "Init Saw", &factory).is_err());
        assert!(compare("Init Saw", &factory, "Init Saw", &factory).is_ok());
    }
}
