//! Links between akter by the kinds their akt types declare (`link.*`).

use kuria_config::LinkDef;

use super::{AktRef, Broken};

/// Refuses a link from an akt to itself.
// r[impl link.add]
pub fn check_not_itself(source: &str, target: &str) -> Result<(), Broken> {
    if source == target {
        return Err(Broken::InvalidLink(format!(
            "`{source}` can't link to itself"
        )));
    }
    Ok(())
}

/// The kind of link `kind` from `source` to `target`, pinning the target's
/// revision `pin` if given, or why it can't be: a kind the source's type
/// doesn't declare, a target type the kind doesn't name, or a revision the
/// target hasn't reached.
// r[impl link.kinds]
// r[impl link.pin]
pub fn to_add<'a>(
    source: &AktRef<'a>,
    kind: &str,
    target: &AktRef<'_>,
    pin: Option<i32>,
) -> Result<&'a LinkDef, Broken> {
    let def = source.ty.link(kind).ok_or_else(|| {
        Broken::InvalidLink(format!(
            "`{}` has no kind of link `{kind}`",
            source.ty.key()
        ))
    })?;
    if !def.to.iter().any(|key| key == target.ty.key()) {
        return Err(Broken::InvalidLink(format!(
            "a `{kind}` link of `{}` goes to {}, not to `{}`",
            source.ty.key(),
            def.to
                .iter()
                .map(|k| format!("`{k}`"))
                .collect::<Vec<_>>()
                .join(", "),
            target.ty.key()
        )));
    }
    if let Some(pin) = pin
        && !(1..=target.revision).contains(&pin)
    {
        return Err(Broken::InvalidLink(format!(
            "`{}` has no revision {pin}",
            target.number
        )));
    }
    Ok(def)
}

/// Refuses a link the source already has (`new` is false).
// r[impl link.add]
pub fn check_new(new: bool, source: &str, kind: &str, target: &str) -> Result<(), Broken> {
    if new {
        Ok(())
    } else {
        Err(Broken::InvalidLink(format!(
            "`{source}` already has a `{kind}` link to `{target}`"
        )))
    }
}

/// The kind of link `kind` from `source`, to unlink by: a kind a newer
/// version of its type dropped still unlinks, by its key.
// r[impl link.remove]
#[must_use]
pub fn to_remove(source: &AktRef<'_>, kind: &str) -> LinkDef {
    source.ty.link(kind).cloned().unwrap_or_else(|| LinkDef {
        key: kind.to_owned(),
        name: kind.to_owned(),
        inverse: kind.to_owned(),
        to: Vec::new(),
    })
}

/// Refuses unlinking a link there was none of (`existed` is false).
// r[impl link.remove]
pub fn check_existed(existed: bool, source: &str, kind: &str, target: &str) -> Result<(), Broken> {
    if existed {
        Ok(())
    } else {
        Err(Broken::NotLinked {
            source_number: source.to_owned(),
            kind: kind.to_owned(),
            target: target.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use kuria_config::{AktType, AktTypeDocument, validate};
    use serde_json::json;

    use super::*;

    fn task() -> AktType {
        validate(
            serde_json::from_value::<AktTypeDocument>(json!({
                "key": "task", "name": "Task", "number": "T-{seq}", "fields": [],
                "states": [{ "key": "open", "name": "Open", "initial": true }],
                "links": [{ "key": "blocks", "name": "Blocks", "inverse": "Blocked by", "to": ["task"] }]
            }))
            .unwrap(),
        )
        .unwrap()
    }

    // r[verify link.kinds]
    // r[verify link.pin]
    // r[verify link.remove]
    #[test]
    fn checks_kinds_targets_and_pins() {
        let ty = task();
        let a = AktRef {
            number: "T-1",
            state: "open",
            ty: &ty,
            revision: 1,
        };
        let b = AktRef {
            revision: 3,
            number: "T-2",
            ..a
        };
        assert_eq!(
            to_add(&a, "blocks", &b, Some(3)).unwrap().inverse,
            "Blocked by"
        );
        assert!(matches!(
            to_add(&a, "relates", &b, None),
            Err(Broken::InvalidLink(_))
        ));
        assert!(matches!(
            to_add(&a, "blocks", &b, Some(4)),
            Err(Broken::InvalidLink(_))
        ));
        assert!(check_not_itself("T-1", "T-1").is_err());
        assert!(check_new(false, "T-1", "blocks", "T-2").is_err());
        assert_eq!(to_remove(&a, "gone").key, "gone");
        assert!(matches!(
            check_existed(false, "T-1", "blocks", "T-2"),
            Err(Broken::NotLinked { .. })
        ));
    }
}
