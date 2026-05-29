#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

#[doc = r" Error types."]
pub mod error {
    #[doc = r" Error from a `TryFrom` or `FromStr` implementation."]
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
#[doc = "symify configuration — a single symify.toml / conf.d/*.toml file. Source of truth for both the Rust config types (via typify) and editor TOML validation. See specs/ARCHITECTURE.md."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"$id\": \"https://github.com/six5536/symify/schema/symify.schema.json\","]
#[doc = "  \"title\": \"Config\","]
#[doc = "  \"description\": \"symify configuration — a single symify.toml / conf.d/*.toml file. Source of truth for both the Rust config types (via typify) and editor TOML validation. See specs/ARCHITECTURE.md.\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"mappings\": {"]
#[doc = "      \"description\": \"Map of mapping name -> mapping.\","]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"additionalProperties\": {"]
#[doc = "        \"$ref\": \"#/$defs/Mapping\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"settings\": {"]
#[doc = "      \"$ref\": \"#/$defs/Settings\""]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"additionalProperties\": false"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[doc = "Map of mapping name -> mapping."]
    #[serde(
        default,
        skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
    )]
    pub mappings: ::std::collections::HashMap<::std::string::String, Mapping>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub settings: ::std::option::Option<Settings>,
}
impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            mappings: Default::default(),
            settings: Default::default(),
        }
    }
}
#[doc = "What to do when the side being written already exists and differs."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Conflict\","]
#[doc = "  \"description\": \"What to do when the side being written already exists and differs.\","]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"skip\","]
#[doc = "    \"replace\","]
#[doc = "    \"backup\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum Conflict {
    #[serde(rename = "skip")]
    Skip,
    #[serde(rename = "replace")]
    Replace,
    #[serde(rename = "backup")]
    Backup,
}
impl ::std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Skip => f.write_str("skip"),
            Self::Replace => f.write_str("replace"),
            Self::Backup => f.write_str("backup"),
        }
    }
}
impl ::std::str::FromStr for Conflict {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "skip" => Ok(Self::Skip),
            "replace" => Ok(Self::Replace),
            "backup" => Ok(Self::Backup),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Conflict {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for Conflict {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Conflict {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "A link entry value: \"\" or true mirrors the key under store; \"<path>\" is an explicit store path (relative to store, or absolute); false disables the entry."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"LinkValue\","]
#[doc = "  \"description\": \"A link entry value: \\\"\\\" or true mirrors the key under store; \\\"<path>\\\" is an explicit store path (relative to store, or absolute); false disables the entry.\","]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"type\": \"boolean\""]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Eq, PartialEq)]
#[serde(untagged)]
pub enum LinkValue {
    String(::std::string::String),
    Boolean(bool),
}
impl ::std::fmt::Display for LinkValue {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::String(x) => x.fmt(f),
            Self::Boolean(x) => x.fmt(f),
        }
    }
}
impl ::std::convert::From<bool> for LinkValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
#[doc = "A named group of links, with optional per-group overrides."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Mapping\","]
#[doc = "  \"description\": \"A named group of links, with optional per-group overrides.\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"conflict\": {"]
#[doc = "      \"$ref\": \"#/$defs/Conflict\""]
#[doc = "    },"]
#[doc = "    \"links\": {"]
#[doc = "      \"description\": \"Map of live-relative (or absolute) key -> link value.\","]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"additionalProperties\": {"]
#[doc = "        \"$ref\": \"#/$defs/LinkValue\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"live\": {"]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"mode\": {"]
#[doc = "      \"$ref\": \"#/$defs/Mode\""]
#[doc = "    },"]
#[doc = "    \"store\": {"]
#[doc = "      \"type\": \"string\""]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"additionalProperties\": false"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub conflict: ::std::option::Option<Conflict>,
    #[doc = "Map of live-relative (or absolute) key -> link value."]
    #[serde(
        default,
        skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
    )]
    pub links: ::std::collections::HashMap<::std::string::String, LinkValue>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub live: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mode: ::std::option::Option<Mode>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub store: ::std::option::Option<::std::string::String>,
}
impl ::std::default::Default for Mapping {
    fn default() -> Self {
        Self {
            conflict: Default::default(),
            links: Default::default(),
            live: Default::default(),
            mode: Default::default(),
            store: Default::default(),
        }
    }
}
#[doc = "Link mechanism."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Mode\","]
#[doc = "  \"description\": \"Link mechanism.\","]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"enum\": ["]
#[doc = "    \"symlink\","]
#[doc = "    \"hardlink\","]
#[doc = "    \"sync\""]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum Mode {
    #[serde(rename = "symlink")]
    Symlink,
    #[serde(rename = "hardlink")]
    Hardlink,
    #[serde(rename = "sync")]
    Sync,
}
impl ::std::fmt::Display for Mode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Symlink => f.write_str("symlink"),
            Self::Hardlink => f.write_str("hardlink"),
            Self::Sync => f.write_str("sync"),
        }
    }
}
impl ::std::str::FromStr for Mode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "symlink" => Ok(Self::Symlink),
            "hardlink" => Ok(Self::Hardlink),
            "sync" => Ok(Self::Sync),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Mode {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for Mode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Mode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Defaults applied to every mapping; each mapping may override them."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"Settings\","]
#[doc = "  \"description\": \"Defaults applied to every mapping; each mapping may override them.\","]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"properties\": {"]
#[doc = "    \"conflict\": {"]
#[doc = "      \"$ref\": \"#/$defs/Conflict\""]
#[doc = "    },"]
#[doc = "    \"live\": {"]
#[doc = "      \"description\": \"Working location where links/copies appear (e.g. \\\"~\\\").\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"mode\": {"]
#[doc = "      \"$ref\": \"#/$defs/Mode\""]
#[doc = "    },"]
#[doc = "    \"store\": {"]
#[doc = "      \"description\": \"Backing repository that holds the real content (e.g. \\\"~/dotfiles\\\").\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"additionalProperties\": false"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub conflict: ::std::option::Option<Conflict>,
    #[doc = "Working location where links/copies appear (e.g. \"~\")."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub live: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mode: ::std::option::Option<Mode>,
    #[doc = "Backing repository that holds the real content (e.g. \"~/dotfiles\")."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub store: ::std::option::Option<::std::string::String>,
}
impl ::std::default::Default for Settings {
    fn default() -> Self {
        Self {
            conflict: Default::default(),
            live: Default::default(),
            mode: Default::default(),
            store: Default::default(),
        }
    }
}
