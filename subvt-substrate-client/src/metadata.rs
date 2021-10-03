/// Substrate metadata. Most of this code has been adopted from [SubXT](https://github.com/paritytech/substrate-subxt).
/// Modified, diminished and augmented as needed.

use core::convert::TryInto;
use frame_metadata::{
    decode_different::DecodeDifferent,
    RuntimeMetadata,
    RuntimeMetadataPrefixed,
};
use log::debug;
use parity_scale_codec::{
    Decode,
    Encode,
    Error as CodecError,
};
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    str::FromStr,
};
use sp_version::RuntimeVersion;
use std::fmt::{Display, Formatter};

/// Metadata error.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    /// Constant is not in metadata.
    #[error("Constant {0} not found")]
    ModuleNotFound(String),
    /// Failed to parse metadata.
    #[error("Error converting substrate metadata: {0}")]
    Conversion(#[from] ConversionError),
    /// Failure to decode constant value.
    #[error("Failed to decode constant value: {0}")]
    ConstantValueError(CodecError),
    /// Constant is not in metadata.
    #[error("Constant {0} not found")]
    ConstantNotFound(String),
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeConfig {
    pub expected_block_time_millis: u64,
    pub epoch_duration_blocks: u64,
    pub epoch_duration_millis: u64,
    pub sessions_per_era: u32,
    pub max_nominations: u32,
    pub era_duration_blocks: u64,
    pub era_duration_millis: u64,
    pub spec_name: String,
    pub spec_version: u32,
    pub tx_version: u32,
}

pub enum MetadataVersion {
    V12,
    V13,
}

/// Runtime metadata.
pub struct Metadata {
    pub version: MetadataVersion,
    pub modules: HashMap<u8, ModuleMetadata>,
    pub runtime_config: RuntimeConfig,
}

impl Metadata {
    pub fn from(hex_string: &str) -> anyhow::Result<Metadata> {
        let metadata_hex_string = hex_string.trim_start_matches("0x");
        let mut metadata_hex_decoded: &[u8] = &hex::decode(&metadata_hex_string)?;
        let metadata_prefixed: RuntimeMetadataPrefixed = RuntimeMetadataPrefixed::decode(
            &mut metadata_hex_decoded
        )?;
        let mut metadata: Metadata = metadata_prefixed.try_into()?;
        let babe_module = metadata.module("Babe")?;
        let expected_block_time_millis: u64 = babe_module.constant("ExpectedBlockTime")?.value()?;
        let epoch_duration_blocks: u64 = babe_module.constant("EpochDuration")?.value()?;
        let epoch_duration_millis: u64 = epoch_duration_blocks * expected_block_time_millis;
        // staking
        let staking_module = metadata.module("Staking")?;
        let sessions_per_era: u32 = staking_module.constant("SessionsPerEra")?.value()?;
        let max_nominations: u32 = staking_module.constant("MaxNominations")?.value()?;
        let era_duration_blocks = epoch_duration_blocks * sessions_per_era as u64;
        let era_duration_millis = era_duration_blocks * expected_block_time_millis;
        // system
        let system_module = metadata.module("System")?;
        let version: RuntimeVersion = system_module.constant("Version")?.value()?;
        metadata.runtime_config = RuntimeConfig {
            expected_block_time_millis,
            epoch_duration_blocks,
            epoch_duration_millis,
            sessions_per_era,
            max_nominations,
            era_duration_blocks,
            era_duration_millis,
            spec_name: String::from(version.spec_name),
            spec_version: version.spec_version,
            tx_version: version.transaction_version,
        };
        metadata.check_event_primitive_argument_support()?;
        Ok(metadata)
    }

    pub fn module(&self, key: &str) -> Result<&ModuleMetadata, MetadataError> {
        self.modules.values()
            .find(|module| module.name == key)
            .ok_or_else(|| MetadataError::ModuleNotFound(key.to_string()))
    }
}

impl Metadata {
    pub fn log_all_calls(&self) {
        let mut call_primitive_arg_name_set: HashSet<String> = HashSet::default();
        debug!("METADATA ALL CALLS");
        debug!("==========================================================");
        for module in self.modules.values() {
            for call in module.calls.values() {
                let mut call_spec = module.name.clone() + "." + &call.name + "(";
                for arg in &call.arguments {
                    call_spec.push_str(&arg.to_string());
                    call_spec.push_str(", ");
                    if let ArgumentMeta::Primitive(name) = arg {
                        call_primitive_arg_name_set.insert(name.clone());
                    }
                }
                call_spec.push(')');
                debug!("{}", call_spec);
            }
        }
        debug!("==========================================================");
        debug!("METADATA CALL ARGUMENTS PRIMITIVE SET");
        debug!("==========================================================");
        for (index, event_arg_name) in call_primitive_arg_name_set.iter().enumerate() {
            debug!("#{} {}", index + 1, event_arg_name);
        }
        debug!("==========================================================");
    }

    pub fn log_all_events(&self) {
        let mut event_primitive_arg_name_set: HashSet<String> = HashSet::default();
        debug!("METADATA ALL EVENTS");
        debug!("==========================================================");
        for module in self.modules.values() {
            for event in module.events.values() {
                let mut event_spec = format!("{}.{}(", module.name, event.name);
                for arg in &event.arguments {
                    event_spec.push_str(&arg.to_string());
                    event_spec.push_str(", ");
                    if let ArgumentMeta::Primitive(name) = arg {
                        event_primitive_arg_name_set.insert(name.clone());
                    }
                }
                event_spec.push(')');
                debug!("{}", event_spec);
            }
        }
        debug!("==========================================================");
        debug!("PRIMITIVE EVENT ARG SET");
        debug!("==========================================================");
        for (index, event_arg_name) in event_primitive_arg_name_set.iter().enumerate() {
            debug!("#{} {}", index + 1, event_arg_name);
        }
        debug!("==========================================================");
    }

    pub fn check_event_primitive_argument_support(&self) -> Result<(), crate::argument::ArgumentDecodeError> {
        debug!("Checking SubVT runtime for event primitive argument support...");
        let mut event_primitive_arg_name_set: HashSet<String> = HashSet::default();
        for module in self.modules.values() {
            for event in module.events.values() {
                for arg in &event.arguments {
                    if let ArgumentMeta::Primitive(name) = arg {
                        event_primitive_arg_name_set.insert(name.clone());
                    }
                }
            }
        }
        for event_arg_name in event_primitive_arg_name_set.iter() {
            let argument_meta = ArgumentMeta::Primitive(event_arg_name.to_string());
            let empty_bytes: Vec<u8> = Vec::new();
            let result = crate::argument::Argument::decode(
                &argument_meta,
                &mut empty_bytes.as_ref(),
            );
            if let Err(error) = result {
                if let crate::argument::ArgumentDecodeError::UnknownPrimitiveType(_) = error {
                    return Err(error);
                }
            }
        }
        Ok(())
    }
}

pub struct ModuleMetadata {
    pub index: u8,
    pub name: String,
    pub storage: HashMap<String, StorageMetadata>,
    pub constants: HashMap<String, ModuleConstantMetadata>,
    pub calls: HashMap<u8, ModuleCallMetadata>,
    pub events: HashMap<u8, ModuleEventMetadata>,
    pub errors: HashMap<u8, String>,
}

impl ModuleMetadata {
    /// Get a constant's metadata by name.
    pub fn constant(
        &self,
        key: &str,
    ) -> Result<&ModuleConstantMetadata, MetadataError> {
        self.constants
            .get(key)
            .ok_or_else(|| MetadataError::ConstantNotFound(key.to_string()))
    }

    pub fn _events(&self) -> impl Iterator<Item=&ModuleEventMetadata> {
        self.events.values()
    }
}

pub enum StorageEntryModifier {
    Optional,
    Default,
}

impl From<frame_metadata::v13::StorageEntryModifier> for StorageEntryModifier {
    fn from(modifier: frame_metadata::v13::StorageEntryModifier) -> Self {
        match modifier {
            frame_metadata::v13::StorageEntryModifier::Default => {
                StorageEntryModifier::Default
            }
            frame_metadata::v13::StorageEntryModifier::Optional => {
                StorageEntryModifier::Optional
            }
        }
    }
}

impl From<frame_metadata::v12::StorageEntryModifier> for StorageEntryModifier {
    fn from(modifier: frame_metadata::v12::StorageEntryModifier) -> Self {
        match modifier {
            frame_metadata::v12::StorageEntryModifier::Default => {
                StorageEntryModifier::Default
            }
            frame_metadata::v12::StorageEntryModifier::Optional => {
                StorageEntryModifier::Optional
            }
        }
    }
}

pub enum StorageHasher {
    V12(frame_metadata::v12::StorageHasher),
    V13(frame_metadata::v13::StorageHasher),
}

pub enum StorageEntryType {
    V12(frame_metadata::v12::StorageEntryType),
    V13(frame_metadata::v13::StorageEntryType),
}

pub struct StorageMetadata {
    pub module_prefix: String,
    pub storage_prefix: String,
    pub modifier: StorageEntryModifier,
    pub ty: StorageEntryType,
    pub default: Vec<u8>,
}

impl StorageMetadata {
    pub fn hash(hasher: &StorageHasher, bytes: &[u8]) -> Vec<u8> {
        match hasher {
            StorageHasher::V12(hasher) => v12::hash(hasher, bytes),
            StorageHasher::V13(hasher) => v13::hash(hasher, bytes),
        }
    }

    pub fn hash_key<K: Encode>(hasher: &StorageHasher, key: &K) -> Vec<u8> {
        Self::hash(hasher, &key.encode())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ModuleCallMetadata {
    pub index: usize,
    pub name: String,
    pub arguments: Vec<ArgumentMeta>,
    pub documentation: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ModuleEventMetadata {
    pub index: usize,
    pub name: String,
    pub arguments: Vec<ArgumentMeta>,
    pub documentation: Vec<String>,
}

/// Naive representation of event argument types, supports current set of substrate EventArg types.
/// If and when Substrate uses `type-metadata`, this can be replaced.
///
/// Used to calculate the size of a instance of an event variant without having the concrete type,
/// so the raw bytes can be extracted from the encoded `Vec<EventRecord<E>>` (without `E` defined).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArgumentMeta {
    Primitive(String),
    Vec(Box<ArgumentMeta>),
    Tuple(Vec<ArgumentMeta>),
    Option(Box<ArgumentMeta>),
}

impl Display for ArgumentMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgumentMeta::Primitive(name) => write!(f, "{}", name),
            ArgumentMeta::Vec(arg) => write!(f, "Vec<{}>", arg),
            ArgumentMeta::Tuple(args) => {
                write!(f, "(")?;
                for arg in args {
                    write!(f, "{}, ", arg)?;
                }
                write!(f, ")")
            }
            ArgumentMeta::Option(arg) => write!(f, "Option<{}>", arg),
        }
    }
}

impl FromStr for ArgumentMeta {
    type Err = ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("Vec<") {
            if s.ends_with('>') {
                Ok(ArgumentMeta::Vec(Box::new(s[4..s.len() - 1].parse()?)))
            } else {
                Err(ConversionError::InvalidEventArg(
                    s.to_string(),
                    "Expected closing `>` for `Vec`",
                ))
            }
        } else if s.starts_with("Option<") {
            if s.ends_with('>') {
                Ok(ArgumentMeta::Option(Box::new(s[7..s.len() - 1].parse()?)))
            } else {
                Err(ConversionError::InvalidEventArg(
                    s.to_string(),
                    "Expected closing `>` for `Option`",
                ))
            }
        } else if s.starts_with('(') {
            if s.ends_with(')') {
                let mut args = Vec::new();
                for arg in s[1..s.len() - 1].split(',') {
                    let arg = arg.trim().parse()?;
                    args.push(arg)
                }
                Ok(ArgumentMeta::Tuple(args))
            } else {
                Err(ConversionError::InvalidEventArg(
                    s.to_string(),
                    "Expecting closing `)` for tuple",
                ))
            }
        } else {
            Ok(ArgumentMeta::Primitive(s.to_string()))
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModuleConstantMetadata {
    name: String,
    ty: String,
    value: Vec<u8>,
    documentation: Vec<String>,
}

impl ModuleConstantMetadata {
    /// Constant value (decoded)
    pub fn value<V: Decode>(&self) -> Result<V, MetadataError> {
        Decode::decode(&mut &self.value[..]).map_err(MetadataError::ConstantValueError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("Invalid prefix")]
    InvalidPrefix,
    #[error("Invalid version")]
    InvalidVersion,
    #[error("Expected DecodeDifferent::Decoded")]
    ExpectedDecoded,
    #[error("Invalid event arg {0}")]
    InvalidEventArg(String, &'static str),
}

impl TryFrom<RuntimeMetadataPrefixed> for Metadata {
    type Error = MetadataError;

    fn try_from(metadata: RuntimeMetadataPrefixed) -> Result<Self, Self::Error> {
        match metadata.1 {
            RuntimeMetadata::V12(meta) => {
                if metadata.0 != frame_metadata::v12::META_RESERVED {
                    return Err(ConversionError::InvalidPrefix.into());
                }
                Ok(Metadata {
                    version: MetadataVersion::V12,
                    modules: v12::convert_modules(meta)?,
                    runtime_config: Default::default(),
                })
            }
            RuntimeMetadata::V13(meta) => {
                if metadata.0 != frame_metadata::v13::META_RESERVED {
                    return Err(ConversionError::InvalidPrefix.into());
                }
                Ok(Metadata {
                    version: MetadataVersion::V13,
                    modules: v13::convert_modules(meta)?,
                    runtime_config: Default::default(),
                })
            }
            _ => Err(ConversionError::InvalidVersion.into()),
        }
    }
}

fn convert<B: 'static, O: 'static>(
    dd: DecodeDifferent<B, O>,
) -> Result<O, ConversionError> {
    match dd {
        DecodeDifferent::Decoded(value) => Ok(value),
        _ => Err(ConversionError::ExpectedDecoded),
    }
}

mod v12 {
    use frame_metadata::v12::{RuntimeMetadataV12};
    use super::{
        ConversionError, convert, ArgumentMeta, ModuleEventMetadata, ModuleCallMetadata,
        ModuleConstantMetadata, ModuleMetadata, StorageEntryType, StorageMetadata,
    };
    use std::collections::HashMap;

    fn convert_event(
        index: usize,
        event: frame_metadata::v12::EventMetadata,
    ) -> Result<ModuleEventMetadata, ConversionError> {
        let name = convert(event.name)?;
        let mut arguments = Vec::new();
        for arg in convert(event.arguments)? {
            let arg = arg.parse::<ArgumentMeta>()?;
            arguments.push(arg);
        }
        let documentation: Vec<String> = convert(event.documentation)?;
        Ok(ModuleEventMetadata { index, name, arguments, documentation })
    }

    fn convert_entry(
        module_prefix: String,
        storage_prefix: String,
        entry: frame_metadata::v12::StorageEntryMetadata,
    ) -> Result<StorageMetadata, ConversionError> {
        let default = convert(entry.default)?;
        Ok(StorageMetadata {
            module_prefix,
            storage_prefix,
            modifier: entry.modifier.into(),
            ty: StorageEntryType::V12(entry.ty),
            default,
        })
    }

    fn convert_error(
        error: frame_metadata::v12::ErrorMetadata,
    ) -> Result<String, ConversionError> {
        convert(error.name)
    }

    fn convert_constant(
        constant: frame_metadata::v12::ModuleConstantMetadata,
    ) -> Result<ModuleConstantMetadata, ConversionError> {
        let name = convert(constant.name)?;
        let ty = convert(constant.ty)?;
        let value = convert(constant.value)?;
        let documentation = convert(constant.documentation)?;
        Ok(ModuleConstantMetadata {
            name,
            ty,
            value,
            documentation,
        })
    }

    pub fn convert_modules(meta: RuntimeMetadataV12) -> Result<HashMap<u8, ModuleMetadata>, super::MetadataError> {
        let mut modules = HashMap::new();
        for module in convert(meta.modules)?.into_iter() {
            let module_index = module.index;
            let module_name = convert(module.name.clone())?.to_string();

            // constants
            let mut constant_map = HashMap::new();
            for constant in convert(module.constants)?.into_iter() {
                let constant_meta = convert_constant(constant)?;
                constant_map.insert(constant_meta.name.clone(), constant_meta);
            }

            // storage
            let mut storage_map = HashMap::new();
            if let Some(storage) = module.storage {
                let storage = convert(storage)?;
                let module_prefix = convert(storage.prefix)?.to_string();
                for entry in convert(storage.entries)?.into_iter() {
                    let storage_prefix = convert(entry.name.clone())?.to_string();
                    let entry = convert_entry(
                        module_prefix.clone(),
                        storage_prefix.clone(),
                        entry,
                    )?;
                    storage_map.insert(storage_prefix, entry);
                }
            }
            // calls
            let mut call_map = HashMap::new();
            if let Some(calls) = module.calls {
                for (index, module_call) in convert(calls)?.into_iter().enumerate() {
                    let mut call = ModuleCallMetadata {
                        name: convert(module_call.name)?,
                        documentation: convert(module_call.documentation)?,
                        ..Default::default()
                    };
                    let arguments = convert(module_call.arguments)?;
                    for module_argument in arguments {
                        let ty = convert(module_argument.ty)?;
                        let argument: ArgumentMeta = ty.parse()?;
                        // arguments.push(arg);
                        call.arguments.push(argument);
                    }
                    call_map.insert(index as u8, call);
                }
            }
            // events
            let mut event_map = HashMap::new();
            if let Some(events) = module.event {
                for (index, event) in convert(events)?.into_iter().enumerate() {
                    event_map.insert(index as u8, convert_event(index, event)?);
                }
            }

            let mut error_map = HashMap::new();
            for (index, error) in convert(module.errors)?.into_iter().enumerate() {
                error_map.insert(index as u8, convert_error(error)?);
            }

            modules.insert(
                module_index,
                ModuleMetadata {
                    index: module.index,
                    name: module_name.clone(),
                    storage: storage_map,
                    constants: constant_map,
                    calls: call_map,
                    events: event_map,
                    errors: error_map,
                },
            );
        }
        Ok(modules)
    }

    pub fn hash(hasher: &frame_metadata::v12::StorageHasher, bytes: &[u8]) -> Vec<u8> {
        match hasher {
            frame_metadata::v12::StorageHasher::Identity => bytes.to_vec(),
            frame_metadata::v12::StorageHasher::Blake2_128 => sp_core::blake2_128(bytes).to_vec(),
            frame_metadata::v12::StorageHasher::Blake2_128Concat => {
                sp_core::blake2_128(bytes)
                    .iter()
                    .chain(bytes)
                    .cloned()
                    .collect()
            }
            frame_metadata::v12::StorageHasher::Blake2_256 => sp_core::blake2_256(bytes).to_vec(),
            frame_metadata::v12::StorageHasher::Twox128 => sp_core::twox_128(bytes).to_vec(),
            frame_metadata::v12::StorageHasher::Twox256 => sp_core::twox_256(bytes).to_vec(),
            frame_metadata::v12::StorageHasher::Twox64Concat => {
                sp_core::twox_64(bytes)
                    .iter()
                    .chain(bytes)
                    .cloned()
                    .collect()
            }
        }
    }
}

mod v13 {
    use frame_metadata::v13::{RuntimeMetadataV13, StorageHasher};
    use super::{
        ConversionError, convert, ArgumentMeta, ModuleEventMetadata, ModuleCallMetadata,
        ModuleConstantMetadata, ModuleMetadata, StorageEntryType, StorageMetadata,
    };
    use std::collections::HashMap;

    fn convert_event(
        index: usize,
        event: frame_metadata::v13::EventMetadata,
    ) -> Result<ModuleEventMetadata, ConversionError> {
        let name = convert(event.name)?;
        let mut arguments = Vec::new();
        for arg in convert(event.arguments)? {
            let arg = arg.parse::<ArgumentMeta>()?;
            arguments.push(arg);
        }
        let documentation: Vec<String> = convert(event.documentation)?;
        Ok(ModuleEventMetadata { index, name, arguments, documentation })
    }

    fn convert_entry(
        module_prefix: String,
        storage_prefix: String,
        entry: frame_metadata::v13::StorageEntryMetadata,
    ) -> Result<StorageMetadata, ConversionError> {
        let default = convert(entry.default)?;
        Ok(StorageMetadata {
            module_prefix,
            storage_prefix,
            modifier: entry.modifier.into(),
            ty: StorageEntryType::V13(entry.ty),
            default,
        })
    }

    fn convert_error(
        error: frame_metadata::v13::ErrorMetadata,
    ) -> Result<String, ConversionError> {
        convert(error.name)
    }

    fn convert_constant(
        constant: frame_metadata::v13::ModuleConstantMetadata,
    ) -> Result<ModuleConstantMetadata, ConversionError> {
        let name = convert(constant.name)?;
        let ty = convert(constant.ty)?;
        let value = convert(constant.value)?;
        let documentation = convert(constant.documentation)?;
        Ok(ModuleConstantMetadata {
            name,
            ty,
            value,
            documentation,
        })
    }

    pub fn convert_modules(meta: RuntimeMetadataV13) -> Result<HashMap<u8, ModuleMetadata>, super::MetadataError> {
        let mut modules = HashMap::new();
        for module in convert(meta.modules)?.into_iter() {
            let module_index = module.index;
            let module_name = convert(module.name.clone())?.to_string();

            // constants
            let mut constant_map = HashMap::new();
            for constant in convert(module.constants)?.into_iter() {
                let constant_meta = convert_constant(constant)?;
                constant_map.insert(constant_meta.name.clone(), constant_meta);
            }

            // storage
            let mut storage_map = HashMap::new();
            if let Some(storage) = module.storage {
                let storage = convert(storage)?;
                let module_prefix = convert(storage.prefix)?.to_string();
                for entry in convert(storage.entries)?.into_iter() {
                    let storage_prefix = convert(entry.name.clone())?.to_string();
                    let entry = convert_entry(
                        module_prefix.clone(),
                        storage_prefix.clone(),
                        entry,
                    )?;
                    storage_map.insert(storage_prefix, entry);
                }
            }
            // calls
            let mut call_map = HashMap::new();
            if let Some(calls) = module.calls {
                for (index, module_call) in convert(calls)?.into_iter().enumerate() {
                    let mut call = ModuleCallMetadata {
                        name: convert(module_call.name)?,
                        documentation: convert(module_call.documentation)?,
                        ..Default::default()
                    };
                    let arguments = convert(module_call.arguments)?;
                    for module_argument in arguments {
                        let ty = convert(module_argument.ty)?;
                        let argument: ArgumentMeta = ty.parse()?;
                        // arguments.push(arg);
                        call.arguments.push(argument);
                    }
                    call_map.insert(index as u8, call);
                }
            }
            // events
            let mut event_map = HashMap::new();
            if let Some(events) = module.event {
                for (index, event) in convert(events)?.into_iter().enumerate() {
                    event_map.insert(index as u8, convert_event(index, event)?);
                }
            }

            let mut error_map = HashMap::new();
            for (index, error) in convert(module.errors)?.into_iter().enumerate() {
                error_map.insert(index as u8, convert_error(error)?);
            }

            modules.insert(
                module_index,
                ModuleMetadata {
                    index: module.index,
                    name: module_name.clone(),
                    storage: storage_map,
                    constants: constant_map,
                    calls: call_map,
                    events: event_map,
                    errors: error_map,
                },
            );
        }
        Ok(modules)
    }

    pub fn hash(hasher: &StorageHasher, bytes: &[u8]) -> Vec<u8> {
        match hasher {
            StorageHasher::Identity => bytes.to_vec(),
            StorageHasher::Blake2_128 => sp_core::blake2_128(bytes).to_vec(),
            StorageHasher::Blake2_128Concat => {
                sp_core::blake2_128(bytes)
                    .iter()
                    .chain(bytes)
                    .cloned()
                    .collect()
            }
            StorageHasher::Blake2_256 => sp_core::blake2_256(bytes).to_vec(),
            StorageHasher::Twox128 => sp_core::twox_128(bytes).to_vec(),
            StorageHasher::Twox256 => sp_core::twox_256(bytes).to_vec(),
            StorageHasher::Twox64Concat => {
                sp_core::twox_64(bytes)
                    .iter()
                    .chain(bytes)
                    .cloned()
                    .collect()
            }
        }
    }
}