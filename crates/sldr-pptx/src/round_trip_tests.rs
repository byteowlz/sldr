#[cfg(test)]
mod tests {
    use crate::{import, build_deck, SlideInput, ZoneContent, Theme};
    use sldr_renderer::LayoutRegistry;

    fn theme() -> Theme {
        Theme::from_parts(
            "demo", Some("#0F172A"), Some("#FFF"), Some("#3B82F6"), Some("#F59E0B"),
            Some("#E2E8F0"), Some("#94A3B8"), Some("Inter"), Some("Inter"),
        )
    }

    #[test]
    fn test_round_trip_framed() {
        let reg = LayoutRegistry::builtin();
        let framed = reg.get("framed").unwrap();
        let slides = vec![SlideInput { details: Default::default(),
            layout: framed,
            fields: vec![
                ("headline".into(), ZoneContent::Text("My Title".into())),
                (
                    "content".into(),
                    ZoneContent::Markdown("- first **bold**\n- second".into()),
                ),
                ("footer".into(), ZoneContent::Text("ACME".into())),
            ],
        }];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();

        let imported = import(&bytes).unwrap();
        assert_eq!(imported.len(), 1);
        let s = &imported[0];
        assert_eq!(s.layout, "framed");
        assert_eq!(s.title.as_deref(), Some("My Title"));
        assert_eq!(s.footer.as_deref(), Some("ACME"));
        assert!(s.body.contains("- first **bold**"));
        assert!(s.body.contains("- second"));
    }

    #[test]
    fn test_round_trip_two_cols_markers() {
        let reg = LayoutRegistry::builtin();
        let two = reg.get("two-cols").unwrap();
        let slides = vec![SlideInput { details: Default::default(),
            layout: two,
            fields: vec![
                ("left".into(), ZoneContent::Markdown("- L1".into())),
                ("right".into(), ZoneContent::Markdown("- R1".into())),
            ],
        }];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();
        let imported = import(&bytes).unwrap();
        let s = &imported[0];
        assert_eq!(s.layout, "two-cols");
        assert!(s.body.contains("::left::"));
        assert!(s.body.contains("::right::"));
        assert!(s.body.contains("- L1"));
        assert!(s.body.contains("- R1"));
    }

    #[test]
    fn test_round_trip_rich_text_is_exact() {
        // Every construct the native text mapping claims survives export →
        // import unchanged, so an untouched zone never shows up as an edit.
        let md = "## Why it matters\n\n\
3. third\n4. fourth\n\n\
- bullet with [a link](https://example.com/x)\n  - nested *italic*\n\n\
> a quoted line\n\n\
~~struck~~ and `code` then a break  \nnext line";
        let reg = LayoutRegistry::builtin();
        let slides = vec![SlideInput { details: Default::default(),
            layout: reg.get("default").unwrap(),
            fields: vec![("content".into(), ZoneContent::Markdown(md.into()))],
        }];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();
        let conv = crate::import_with_report(&bytes).unwrap();
        assert!(conv.report.findings.iter().all(|f| f.feature != "unsupported_xml" && f.feature != "unsupported_attribute"),
            "{:?}", conv.report.findings.iter().filter(|f| f.feature.starts_with("unsupported")).collect::<Vec<_>>());
        assert_eq!(conv.value[0].body, md);
        assert!(conv.value[0].zones.iter().all(|z| z.changed == Some(false)));
    }

    #[test]
    fn test_manifest_is_an_office_custom_xml_item() {
        // PowerPoint discards root-related custom parts on save but keeps
        // data-store items related to the presentation part.
        let reg = LayoutRegistry::builtin();
        let slides = vec![SlideInput { details: Default::default(), layout: reg.get("default").unwrap(),
            fields: vec![("content".into(), ZoneContent::Markdown("x".into()))] }];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();
        let pkg = crate::package::Package::read(&bytes).unwrap();
        assert!(pkg.parts.contains_key("customXml/item1.xml"));
        assert!(pkg.parts.contains_key("customXml/itemProps1.xml"));
        let rels = pkg.text("ppt/_rels/presentation.xml.rels").unwrap();
        assert!(rels.contains("../customXml/item1.xml"));
        assert!(!pkg.text("_rels/.rels").unwrap().contains("customXml"));
        crate::validate_package(&bytes).unwrap();
        // An editor renumbering the item (item1 → item3) must not lose it.
        let renamed: Vec<(String, Vec<u8>)> = pkg.parts.iter().map(|(k, v)| {
            let k = k.replace("customXml/item1.xml", "customXml/item3.xml").replace("_rels/item1.xml.rels", "_rels/item3.xml.rels");
            let v = String::from_utf8(v.clone()).map(|t| t.replace("customXml/item1.xml", "customXml/item3.xml").into_bytes()).unwrap_or_else(|e| e.into_bytes());
            (k, v)
        }).collect();
        let text: Vec<(String, String)> = renamed.iter().filter_map(|(k, v)| String::from_utf8(v.clone()).ok().map(|t| (k.clone(), t))).collect();
        let media: Vec<(String, Vec<u8>)> = renamed.iter().filter(|(_, v)| String::from_utf8(v.clone()).is_err()).cloned().collect();
        let bytes = crate::zip_mixed(&text, &media).unwrap();
        let imported = import(&bytes).unwrap();
        assert!(imported[0].identity.is_some(), "manifest found after renumbering");
    }

    #[test]
    fn test_import_rejects_non_sldr_deck() {
        // A zip without the sldr app marker.
        let parts = vec![("docProps/app.xml".to_string(), "<x/>".to_string())];
        let bytes = crate::zip_parts(&parts).unwrap();
        let err = import(&bytes).unwrap_err().to_string();
        assert!(err.contains("not generated by sldr"));
    }

    #[test]
    fn test_round_trip_picture() {
        let reg = LayoutRegistry::builtin();
        let il = reg.get("image-left").unwrap();
        let slides = vec![SlideInput { details: Default::default(),
            layout: il,
            fields: vec![
                ("content".into(), ZoneContent::Markdown("- point".into())),
                (
                    "image".into(),
                    ZoneContent::Picture {
                        bytes: b"\x89PNG fake".to_vec(),
                        ext: "png".into(),
                        fit: None,
                    },
                ),
            ],
        }];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();
        let imported = import(&bytes).unwrap();
        let s = &imported[0];
        assert_eq!(s.images.len(), 1);
        assert!(s.body.contains("::content::"));
        assert!(s.body.contains("::image::"));
    }
}
