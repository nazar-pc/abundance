use crate::metadata::ContractMetadataKind;
use ab_contracts_io_type::metadata::IoTypeMetadataKind;
use core::str;
use core::str::Utf8Error;

#[derive(Debug, thiserror::Error)]
pub enum MethodMetadataDecodingError<'a> {
    #[error("Not enough metadata to decode")]
    NotEnoughMetadata,
    #[error("Invalid first metadata byte")]
    InvalidFirstMetadataByte { byte: u8 },
    #[error("Expected method kind, found something else: {metadata_kind:?}")]
    ExpectedMethodKind { metadata_kind: ContractMetadataKind },
    #[error("Invalid method name {method_name:?}: {error}")]
    InvalidMethodName {
        method_name: &'a [u8],
        error: Utf8Error,
    },
    #[error("Expected argument kind, found something else: {metadata_kind:?}")]
    ExpectedArgumentKind { metadata_kind: ContractMetadataKind },
    #[error("Unexpected argument kind {argument_kind:?} for method kind {method_kind:?}")]
    UnexpectedArgumentKind {
        argument_kind: ArgumentKind,
        method_kind: MethodKind,
    },
    #[error("Invalid argument name {argument_name:?}: {error}")]
    InvalidArgumentName {
        argument_name: &'a [u8],
        error: Utf8Error,
    },
    #[error("Invalid argument I/O type of kind {argument_kind:?} for {argument_name}")]
    InvalidArgumentIoType {
        argument_name: &'a str,
        argument_kind: ArgumentKind,
    },
}

#[derive(Debug, Copy, Clone)]
pub enum MethodKind {
    /// Corresponds to [`ContractMetadataKind::Init`]
    Init,
    /// Corresponds to [`ContractMetadataKind::UpdateStateless`]
    UpdateStateless,
    /// Corresponds to [`ContractMetadataKind::UpdateStatefulRo`]
    UpdateStatefulRo,
    /// Corresponds to [`ContractMetadataKind::UpdateStatefulRw`]
    UpdateStatefulRw,
    /// Corresponds to [`ContractMetadataKind::ViewStateless`]
    ViewStateless,
    /// Corresponds to [`ContractMetadataKind::ViewStatefulRo`]
    ViewStatefulRo,
}

#[derive(Debug, Copy, Clone)]
pub enum ArgumentKind {
    /// Corresponds to [`ContractMetadataKind::EnvRo`]
    EnvRo,
    /// Corresponds to [`ContractMetadataKind::EnvRw`]
    EnvRw,
    /// Corresponds to [`ContractMetadataKind::TmpRo`]
    TmpRo,
    /// Corresponds to [`ContractMetadataKind::TmpRw`]
    TmpRw,
    /// Corresponds to [`ContractMetadataKind::SlotWithAddressRo`]
    SlotWithAddressRo,
    /// Corresponds to [`ContractMetadataKind::SlotWithAddressRw`]
    SlotWithAddressRw,
    /// Corresponds to [`ContractMetadataKind::SlotWithoutAddressRo`]
    SlotWithoutAddressRo,
    /// Corresponds to [`ContractMetadataKind::SlotWithoutAddressRw`]
    SlotWithoutAddressRw,
    /// Corresponds to [`ContractMetadataKind::Input`]
    Input,
    /// Corresponds to [`ContractMetadataKind::Output`]
    Output,
    /// Corresponds to [`ContractMetadataKind::Result`]
    Result,
}

pub struct MethodMetadataItem<'a> {
    pub method_name: &'a str,
    pub method_kind: MethodKind,
    pub num_arguments: u8,
}

pub struct ArgumentMetadataItem<'a> {
    pub argument_name: &'a str,
    pub argument_kind: ArgumentKind,
    pub recommended_capacity: u32,
}

mod private {
    pub trait Stage {}
}

pub struct StageMethod {}

impl private::Stage for StageMethod {}

pub struct StageArguments {
    method_kind: MethodKind,
    remaining: u8,
}

impl private::Stage for StageArguments {}

pub struct MethodMetadataDecoder<'a, Stage>
where
    Stage: private::Stage,
{
    metadata: &'a [u8],
    stage: Stage,
}

impl<'a> MethodMetadataDecoder<'a, StageMethod> {
    pub fn new(metadata: &'a [u8]) -> Self {
        Self {
            metadata,
            stage: StageMethod {},
        }
    }

    pub fn decode_next(
        mut self,
    ) -> Result<
        (
            MethodMetadataDecoder<'a, StageArguments>,
            MethodMetadataItem<'a>,
        ),
        MethodMetadataDecodingError<'a>,
    > {
        if self.metadata.is_empty() {
            return Err(MethodMetadataDecodingError::NotEnoughMetadata);
        }

        // Decode method kind
        let metadata_kind = ContractMetadataKind::try_from_u8(self.metadata[0]).ok_or(
            MethodMetadataDecodingError::InvalidFirstMetadataByte {
                byte: self.metadata[0],
            },
        )?;
        self.metadata = &self.metadata[1..];

        let method_kind = match metadata_kind {
            ContractMetadataKind::Init => MethodKind::Init,
            ContractMetadataKind::UpdateStateless => MethodKind::UpdateStateless,
            ContractMetadataKind::UpdateStatefulRo => MethodKind::UpdateStatefulRo,
            ContractMetadataKind::UpdateStatefulRw => MethodKind::UpdateStatefulRw,
            ContractMetadataKind::ViewStateless => MethodKind::ViewStateless,
            ContractMetadataKind::ViewStatefulRo => MethodKind::ViewStatefulRo,
            // The rest are not methods and can't appear here
            ContractMetadataKind::Contract
            | ContractMetadataKind::Trait
            | ContractMetadataKind::EnvRo
            | ContractMetadataKind::EnvRw
            | ContractMetadataKind::TmpRo
            | ContractMetadataKind::TmpRw
            | ContractMetadataKind::SlotWithAddressRo
            | ContractMetadataKind::SlotWithAddressRw
            | ContractMetadataKind::SlotWithoutAddressRo
            | ContractMetadataKind::SlotWithoutAddressRw
            | ContractMetadataKind::Input
            | ContractMetadataKind::Output
            | ContractMetadataKind::Result => {
                return Err(MethodMetadataDecodingError::ExpectedMethodKind { metadata_kind });
            }
        };

        if self.metadata.is_empty() {
            return Err(MethodMetadataDecodingError::NotEnoughMetadata);
        }

        // Decode method name
        let method_name_length = usize::from(self.metadata[0]);
        self.metadata = &self.metadata[1..];

        // +1 for number of arguments
        if self.metadata.len() < method_name_length + 1 {
            return Err(MethodMetadataDecodingError::NotEnoughMetadata);
        }

        let method_name = &self.metadata[..method_name_length];
        self.metadata = &self.metadata[method_name_length..];
        let method_name = str::from_utf8(method_name).map_err(|error| {
            MethodMetadataDecodingError::InvalidMethodName { method_name, error }
        })?;

        let num_arguments = self.metadata[0];
        self.metadata = &self.metadata[1..];

        let decoder = MethodMetadataDecoder {
            metadata: self.metadata,
            stage: StageArguments {
                method_kind,
                remaining: num_arguments,
            },
        };
        let item = MethodMetadataItem {
            method_name,
            method_kind,
            num_arguments,
        };

        Ok((decoder, item))
    }
}

impl<'a> MethodMetadataDecoder<'a, StageArguments> {
    pub fn decode_next(
        &'a mut self,
    ) -> Option<Result<ArgumentMetadataItem<'a>, MethodMetadataDecodingError<'a>>> {
        if self.stage.remaining == 0 {
            return None;
        }

        self.stage.remaining -= 1;

        Some(self.decode_argument())
    }

    fn decode_argument(
        &'a mut self,
    ) -> Result<ArgumentMetadataItem<'a>, MethodMetadataDecodingError<'a>> {
        if self.metadata.is_empty() {
            return Err(MethodMetadataDecodingError::NotEnoughMetadata);
        }

        // Decode method kind
        let metadata_kind = ContractMetadataKind::try_from_u8(self.metadata[0]).ok_or(
            MethodMetadataDecodingError::InvalidFirstMetadataByte {
                byte: self.metadata[0],
            },
        )?;
        self.metadata = &self.metadata[1..];

        let argument_kind = match metadata_kind {
            ContractMetadataKind::EnvRo => ArgumentKind::EnvRo,
            ContractMetadataKind::EnvRw => ArgumentKind::EnvRw,
            ContractMetadataKind::TmpRo => ArgumentKind::TmpRo,
            ContractMetadataKind::TmpRw => ArgumentKind::TmpRw,
            ContractMetadataKind::SlotWithAddressRo => ArgumentKind::SlotWithAddressRo,
            ContractMetadataKind::SlotWithAddressRw => ArgumentKind::SlotWithAddressRw,
            ContractMetadataKind::SlotWithoutAddressRo => ArgumentKind::SlotWithoutAddressRo,
            ContractMetadataKind::SlotWithoutAddressRw => ArgumentKind::SlotWithoutAddressRw,
            ContractMetadataKind::Input => ArgumentKind::Input,
            ContractMetadataKind::Output => ArgumentKind::Output,
            ContractMetadataKind::Result => ArgumentKind::Result,
            // The rest are not arguments and can't appear here
            ContractMetadataKind::Contract
            | ContractMetadataKind::Trait
            | ContractMetadataKind::Init
            | ContractMetadataKind::UpdateStateless
            | ContractMetadataKind::UpdateStatefulRo
            | ContractMetadataKind::UpdateStatefulRw
            | ContractMetadataKind::ViewStateless
            | ContractMetadataKind::ViewStatefulRo => {
                return Err(MethodMetadataDecodingError::ExpectedArgumentKind { metadata_kind });
            }
        };

        let argument_allowed = match self.stage.method_kind {
            MethodKind::Init
            | MethodKind::UpdateStateless
            | MethodKind::UpdateStatefulRo
            | MethodKind::UpdateStatefulRw => match argument_kind {
                ArgumentKind::EnvRo
                | ArgumentKind::EnvRw
                | ArgumentKind::TmpRo
                | ArgumentKind::TmpRw
                | ArgumentKind::SlotWithAddressRo
                | ArgumentKind::SlotWithAddressRw
                | ArgumentKind::SlotWithoutAddressRo
                | ArgumentKind::SlotWithoutAddressRw
                | ArgumentKind::Input
                | ArgumentKind::Output
                | ArgumentKind::Result => true,
            },
            MethodKind::ViewStateless | MethodKind::ViewStatefulRo => match argument_kind {
                ArgumentKind::EnvRo
                | ArgumentKind::SlotWithAddressRo
                | ArgumentKind::SlotWithoutAddressRo
                | ArgumentKind::Input
                | ArgumentKind::Output
                | ArgumentKind::Result => true,
                ArgumentKind::EnvRw
                | ArgumentKind::TmpRo
                | ArgumentKind::TmpRw
                | ArgumentKind::SlotWithAddressRw
                | ArgumentKind::SlotWithoutAddressRw => false,
            },
        };

        if !argument_allowed {
            return Err(MethodMetadataDecodingError::UnexpectedArgumentKind {
                argument_kind,
                method_kind: self.stage.method_kind,
            });
        }

        let (argument_name, recommended_capacity) = match argument_kind {
            ArgumentKind::EnvRo | ArgumentKind::EnvRw => ("env", 0),
            ArgumentKind::TmpRo
            | ArgumentKind::TmpRw
            | ArgumentKind::SlotWithAddressRo
            | ArgumentKind::SlotWithAddressRw
            | ArgumentKind::SlotWithoutAddressRo
            | ArgumentKind::SlotWithoutAddressRw
            | ArgumentKind::Input
            | ArgumentKind::Output
            | ArgumentKind::Result => {
                if self.metadata.is_empty() {
                    return Err(MethodMetadataDecodingError::NotEnoughMetadata);
                }

                // Decode argument name
                let argument_name_length = usize::from(self.metadata[0]);
                self.metadata = &self.metadata[1..];

                // +1 for number of arguments
                if self.metadata.len() < argument_name_length {
                    return Err(MethodMetadataDecodingError::NotEnoughMetadata);
                }

                let argument_name = &self.metadata[..argument_name_length];
                self.metadata = &self.metadata[argument_name_length..];
                let argument_name = str::from_utf8(argument_name).map_err(|error| {
                    MethodMetadataDecodingError::InvalidArgumentName {
                        argument_name,
                        error,
                    }
                })?;

                let recommended_capacity;
                (recommended_capacity, self.metadata) = IoTypeMetadataKind::recommended_capacity(
                    self.metadata,
                )
                .ok_or(MethodMetadataDecodingError::InvalidArgumentIoType {
                    argument_name,
                    argument_kind,
                })?;

                (argument_name, recommended_capacity)
            }
        };

        Ok(ArgumentMetadataItem {
            argument_name,
            argument_kind,
            recommended_capacity,
        })
    }
}
