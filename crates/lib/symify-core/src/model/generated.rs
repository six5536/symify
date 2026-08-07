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
#[doc = "symify configuration — a single symify.toml / conf.d/*.toml file. Source of truth for both the Rust config types (via typify) and editor TOML validation. See knowledge/configuration.md."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"$id\": \"https://github.com/six5536/symify/schema/symify.schema.json\","]
#[doc = "  \"title\": \"Config\","]
#[doc = "  \"description\": \"symify configuration — a single symify.toml / conf.d/*.toml file. Source of truth for both the Rust config types (via typify) and editor TOML validation. See knowledge/configuration.md.\","]
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
#[doc = "      \"description\": \"Defaults applied to every mapping (each mapping may override them).\","]
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
    #[doc = "Defaults applied to every mapping (each mapping may override them)."]
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
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"description\": \"Leave the existing file and report the conflict (counts as drift).\","]
#[doc = "      \"const\": \"skip\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"description\": \"Delete the existing file, then write (no backup).\","]
#[doc = "      \"const\": \"replace\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"description\": \"Rename the existing file to `<name>.<timestamp>.bak`, then write.\","]
#[doc = "      \"const\": \"backup\""]
#[doc = "    }"]
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
    #[doc = "Leave the existing file and report the conflict (counts as drift)."]
    #[serde(rename = "skip")]
    Skip,
    #[doc = "Delete the existing file, then write (no backup)."]
    #[serde(rename = "replace")]
    Replace,
    #[doc = "Rename the existing file to `<name>.<timestamp>.bak`, then write."]
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
#[doc = "A link entry value: \"\" or true mirrors the key under store; `<path>` is an explicit store path (relative to store, or absolute); false disables the entry."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"LinkValue\","]
#[doc = "  \"description\": \"A link entry value: \\\"\\\" or true mirrors the key under store; `<path>` is an explicit store path (relative to store, or absolute); false disables the entry.\","]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"description\": \"An explicit store path (relative to store, or absolute); \\\"\\\" mirrors the key under store.\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"description\": \"true mirrors the key under store; false disables the entry.\","]
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
#[doc = "A machine condition: one pattern or a list of alternatives. A pattern matches case-insensitively; `*` is allowed at the start and/or end of a host pattern (e.g. \"wrk-*\", \"*.local\"), nowhere else."]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"title\": \"MachineMatch\","]
#[doc = "  \"description\": \"A machine condition: one pattern or a list of alternatives. A pattern matches case-insensitively; `*` is allowed at the start and/or end of a host pattern (e.g. \\\"wrk-*\\\", \\\"*.local\\\"), nowhere else.\","]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"description\": \"A single pattern.\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"description\": \"Alternatives; the condition matches when any pattern matches.\","]
#[doc = "      \"type\": \"array\","]
#[doc = "      \"items\": {"]
#[doc = "        \"type\": \"string\""]
#[doc = "      },"]
#[doc = "      \"minItems\": 1"]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, Eq, PartialEq)]
#[serde(untagged)]
pub enum MachineMatch {
    String(::std::string::String),
    Array(::std::vec::Vec<::std::string::String>),
}
impl ::std::convert::From<::std::vec::Vec<::std::string::String>> for MachineMatch {
    fn from(value: ::std::vec::Vec<::std::string::String>) -> Self {
        Self::Array(value)
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
#[doc = "    \"backup_keep\": {"]
#[doc = "      \"description\": \"Backup retention for this mapping (overrides settings.backup_keep). 0 = keep all.\","]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"minimum\": 0.0"]
#[doc = "    },"]
#[doc = "    \"conflict\": {"]
#[doc = "      \"description\": \"Conflict policy for this mapping (overrides settings.conflict).\","]
#[doc = "      \"$ref\": \"#/$defs/Conflict\""]
#[doc = "    },"]
#[doc = "    \"host\": {"]
#[doc = "      \"description\": \"Hostnames this mapping applies to, matched case-insensitively; `*` may open and/or close a pattern (\\\"wrk-*\\\", \\\"*.local\\\"). On other machines the mapping is inactive. Absent = all.\","]
#[doc = "      \"$ref\": \"#/$defs/MachineMatch\""]
#[doc = "    },"]
#[doc = "    \"links\": {"]
#[doc = "      \"description\": \"Map of live-relative (or absolute) key -> link value.\","]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"additionalProperties\": {"]
#[doc = "        \"$ref\": \"#/$defs/LinkValue\""]
#[doc = "      }"]
#[doc = "    },"]
#[doc = "    \"live\": {"]
#[doc = "      \"description\": \"Working location for this mapping (overrides settings.live).\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"mode\": {"]
#[doc = "      \"description\": \"Link mechanism for this mapping (overrides settings.mode).\","]
#[doc = "      \"$ref\": \"#/$defs/Mode\""]
#[doc = "    },"]
#[doc = "    \"os\": {"]
#[doc = "      \"description\": \"Operating systems this mapping applies to: \\\"linux\\\", \\\"macos\\\" or \\\"windows\\\" (Rust's std::env::consts::OS values), matched case-insensitively with no globs. On other machines the mapping is inactive. Absent = all.\","]
#[doc = "      \"$ref\": \"#/$defs/MachineMatch\""]
#[doc = "    },"]
#[doc = "    \"store\": {"]
#[doc = "      \"description\": \"Backing repository for this mapping (overrides settings.store).\","]
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
    #[doc = "Backup retention for this mapping (overrides settings.backup_keep). 0 = keep all."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub backup_keep: ::std::option::Option<u64>,
    #[doc = "Conflict policy for this mapping (overrides settings.conflict)."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub conflict: ::std::option::Option<Conflict>,
    #[doc = "Hostnames this mapping applies to, matched case-insensitively; `*` may open and/or close a pattern (\"wrk-*\", \"*.local\"). On other machines the mapping is inactive. Absent = all."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub host: ::std::option::Option<MachineMatch>,
    #[doc = "Map of live-relative (or absolute) key -> link value."]
    #[serde(
        default,
        skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
    )]
    pub links: ::std::collections::HashMap<::std::string::String, LinkValue>,
    #[doc = "Working location for this mapping (overrides settings.live)."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub live: ::std::option::Option<::std::string::String>,
    #[doc = "Link mechanism for this mapping (overrides settings.mode)."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mode: ::std::option::Option<Mode>,
    #[doc = "Operating systems this mapping applies to: \"linux\", \"macos\" or \"windows\" (Rust's std::env::consts::OS values), matched case-insensitively with no globs. On other machines the mapping is inactive. Absent = all."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub os: ::std::option::Option<MachineMatch>,
    #[doc = "Backing repository for this mapping (overrides settings.store)."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub store: ::std::option::Option<::std::string::String>,
}
impl ::std::default::Default for Mapping {
    fn default() -> Self {
        Self {
            backup_keep: Default::default(),
            conflict: Default::default(),
            host: Default::default(),
            links: Default::default(),
            live: Default::default(),
            mode: Default::default(),
            os: Default::default(),
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
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"description\": \"A symbolic link at the live path pointing to the real file in the store.\","]
#[doc = "      \"const\": \"symlink\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"description\": \"An independent content copy, kept up to date incrementally (only changed files are copied).\","]
#[doc = "      \"const\": \"copy\""]
#[doc = "    }"]
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
    #[doc = "A symbolic link at the live path pointing to the real file in the store."]
    #[serde(rename = "symlink")]
    Symlink,
    #[doc = "An independent content copy, kept up to date incrementally (only changed files are copied)."]
    #[serde(rename = "copy")]
    Copy,
}
impl ::std::fmt::Display for Mode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Symlink => f.write_str("symlink"),
            Self::Copy => f.write_str("copy"),
        }
    }
}
impl ::std::str::FromStr for Mode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "symlink" => Ok(Self::Symlink),
            "copy" => Ok(Self::Copy),
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
#[doc = "    \"backup_keep\": {"]
#[doc = "      \"description\": \"Keep at most this many `<name>.<timestamp>.bak` backups per path, deleting the oldest when a new backup is written. 0 or absent = keep all (default).\","]
#[doc = "      \"type\": \"integer\","]
#[doc = "      \"minimum\": 0.0"]
#[doc = "    },"]
#[doc = "    \"conflict\": {"]
#[doc = "      \"description\": \"Default policy when the side being written already exists and differs.\","]
#[doc = "      \"$ref\": \"#/$defs/Conflict\""]
#[doc = "    },"]
#[doc = "    \"live\": {"]
#[doc = "      \"description\": \"Working location where links/copies appear (e.g. \\\"~\\\").\","]
#[doc = "      \"type\": \"string\""]
#[doc = "    },"]
#[doc = "    \"mode\": {"]
#[doc = "      \"description\": \"Default link mechanism for all mappings.\","]
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
    #[doc = "Keep at most this many `<name>.<timestamp>.bak` backups per path, deleting the oldest when a new backup is written. 0 or absent = keep all (default)."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub backup_keep: ::std::option::Option<u64>,
    #[doc = "Default policy when the side being written already exists and differs."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub conflict: ::std::option::Option<Conflict>,
    #[doc = "Working location where links/copies appear (e.g. \"~\")."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub live: ::std::option::Option<::std::string::String>,
    #[doc = "Default link mechanism for all mappings."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mode: ::std::option::Option<Mode>,
    #[doc = "Backing repository that holds the real content (e.g. \"~/dotfiles\")."]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub store: ::std::option::Option<::std::string::String>,
}
impl ::std::default::Default for Settings {
    fn default() -> Self {
        Self {
            backup_keep: Default::default(),
            conflict: Default::default(),
            live: Default::default(),
            mode: Default::default(),
            store: Default::default(),
        }
    }
}
