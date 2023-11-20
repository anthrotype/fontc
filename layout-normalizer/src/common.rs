//! general common utilities and types

use std::{collections::BTreeSet, fmt::Display};

use read_fonts::{
    tables::layout::{FeatureList, ScriptList},
    types::{GlyphId, Tag},
};

use crate::glyph_names::NameMap;

/// A set of lookups for a specific feature and language system
pub(crate) struct Feature {
    pub(crate) feature: Tag,
    pub(crate) script: Tag,
    pub(crate) lang: Tag,
    pub(crate) lookups: Vec<u16>,
}

/// A type to represent either one or multiple glyphs
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub(crate) enum GlyphSet {
    Single(GlyphId),
    Multiple(BTreeSet<GlyphId>),
}

impl Feature {
    fn sort_key(&self) -> impl Ord {
        // make it so we always put DFLT/dflt above other tags
        fn tag_to_int(tag: Tag) -> u32 {
            if tag == Tag::new(b"DFLT") {
                0
            } else if tag == Tag::new(b"dflt") {
                1
            } else {
                u32::from_be_bytes(tag.to_be_bytes())
            }
        }

        (
            tag_to_int(self.feature),
            tag_to_int(self.script),
            tag_to_int(self.lang),
        )
    }
}

pub(crate) fn get_lang_systems(
    script_list: &ScriptList,
    feature_list: &FeatureList,
) -> Vec<Feature> {
    let data = script_list.offset_data();

    let mut result = script_list
        .script_records()
        .iter()
        // first iterate all (script, lang, feature indices)
        .flat_map(|script| {
            let script_tag = script.script_tag();
            let script = script.script(data).unwrap();
            let maybe_default = script
                .default_lang_sys()
                .transpose()
                .unwrap()
                .map(|dflt| (script_tag, Tag::new(b"dflt"), dflt.feature_indices()));
            let lang_sys_iter = script.lang_sys_records().iter().map(move |lang_sys| {
                let lang_tag = lang_sys.lang_sys_tag();
                let lang = lang_sys.lang_sys(script.offset_data()).unwrap();
                (script_tag, lang_tag, lang.feature_indices())
            });
            maybe_default.into_iter().chain(lang_sys_iter)
        })
        // then convert these into script/lang/feature/lookup indices
        .flat_map(|(script, lang, indices)| {
            indices.iter().map(move |idx| {
                let rec = feature_list
                    .feature_records()
                    .get(idx.get() as usize)
                    .unwrap();
                let feature = rec.feature(feature_list.offset_data()).unwrap();
                let lookups = feature
                    .lookup_list_indices()
                    .iter()
                    .map(|x| x.get())
                    .collect();
                Feature {
                    feature: rec.feature_tag(),
                    script,
                    lang,
                    lookups,
                }
            })
        })
        .collect::<Vec<_>>();

    result.sort_unstable_by_key(|sys| sys.sort_key());

    result
}

impl GlyphSet {
    pub(crate) fn make_set(&mut self) {
        if let GlyphSet::Single(gid) = self {
            *self = GlyphSet::Multiple(BTreeSet::from([*gid]))
        }
    }

    pub(crate) fn combine(&mut self, other: GlyphSet) {
        self.make_set();
        let GlyphSet::Multiple(gids) = self else {
            unreachable!()
        };
        match other {
            GlyphSet::Single(gid) => {
                gids.insert(gid);
            }
            GlyphSet::Multiple(mut multi) => gids.append(&mut multi),
        }
    }

    pub(crate) fn add(&mut self, gid: GlyphId) {
        // if we're a single glyph, don't turn into a set if we're adding ourselves
        if matches!(self, GlyphSet::Single(x) if *x == gid) {
            return;
        }
        self.make_set();
        if let GlyphSet::Multiple(set) = self {
            set.insert(gid);
        }
    }

    pub(crate) fn printer<'a>(&'a self, names: &'a NameMap) -> impl Display + 'a {
        // A helper for printing one or more glyphs
        struct GlyphPrinter<'a> {
            glyphs: &'a GlyphSet,
            names: &'a NameMap,
        }

        impl Display for GlyphPrinter<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.glyphs {
                    GlyphSet::Single(single) => {
                        let name = self.names.get(*single);
                        f.write_str(name)
                    }
                    GlyphSet::Multiple(glyphs) => {
                        f.write_str("[")?;
                        let mut first = true;
                        for gid in glyphs {
                            let name = self.names.get(*gid);
                            if !first {
                                f.write_str(",")?;
                            }
                            f.write_str(name)?;
                            first = false;
                        }
                        f.write_str("]")
                    }
                }
            }
        }

        GlyphPrinter {
            glyphs: self,
            names,
        }
    }
}

impl From<GlyphId> for GlyphSet {
    fn from(src: GlyphId) -> GlyphSet {
        GlyphSet::Single(src)
    }
}

impl FromIterator<GlyphId> for GlyphSet {
    fn from_iter<T: IntoIterator<Item = GlyphId>>(iter: T) -> Self {
        GlyphSet::Multiple(iter.into_iter().collect())
    }
}