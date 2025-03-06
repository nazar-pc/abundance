use crate::metadata::ContractMetadataKind;
use ab_contracts_io_type::metadata::{IoTypeDetails, IoTypeMetadataKind};
use core::str::Utf8Error;

/// Metadata decoding error
#[derive(Debug, thiserror::Error)]
pub enum MetadataDecodingError<'metadata> {
    /// Not enough metadata to decode
    #[error("Not enough metadata to decode")]
    NotEnoughMetadata,
    /// Invalid first metadata byte
    #[error("Invalid first metadata byte")]
    InvalidFirstMetadataByte { byte: u8 },
    /// Multiple contracts found
    #[error("Multiple contracts found")]
    MultipleContractsFound,
    /// Expected contract or trait kind, found something else
    #[error("Expected contract or trait kind, found something else: {metadata_kind:?}")]
    ExpectedContractOrTrait { metadata_kind: ContractMetadataKind },
    /// Failed to decode state type name
    #[error("Failed to decode state type name")]
    FailedToDecodeStateTypeName,
    /// Invalid state I/O type
    #[error("Invalid state I/O type")]
    InvalidStateIoType,
    /// Invalid trait name
    #[error("Invalid trait name {trait_name:?}: {error}")]
    InvalidTraitName {
        trait_name: &'metadata [u8],
        error: Utf8Error,
    },
    /// Unexpected method kind
    #[error("Unexpected method kind {method_kind:?} for container kind {container_kind:?}")]
    UnexpectedMethodKind {
        method_kind: MethodKind,
        container_kind: MethodsContainerKind,
    },
    /// Expected method kind, found something else
    #[error("Expected method kind, found something else: {metadata_kind:?}")]
    ExpectedMethodKind { metadata_kind: ContractMetadataKind },
    /// Invalid method name
    #[error("Invalid method name {method_name:?}: {error}")]
    InvalidMethodName {
        method_name: &'metadata [u8],
        error: Utf8Error,
    },
    /// Expected argument kind, found something else
    #[error("Expected argument kind, found something else: {metadata_kind:?}")]
    ExpectedArgumentKind { metadata_kind: ContractMetadataKind },
    /// Unexpected argument kind
    #[error("Unexpected argument kind {argument_kind:?} for method kind {method_kind:?}")]
    UnexpectedArgumentKind {
        argument_kind: ArgumentKind,
        method_kind: MethodKind,
    },
    /// Invalid argument name
    #[error("Invalid argument name {argument_name:?}: {error}")]
    InvalidArgumentName {
        argument_name: &'metadata [u8],
        error: Utf8Error,
    },
    /// Invalid argument I/O type
    #[error("Invalid argument I/O type of kind {argument_kind:?} for {argument_name}")]
    InvalidArgumentIoType {
        argument_name: &'metadata str,
        argument_kind: ArgumentKind,
    },
}

#[derive(Debug)]
pub enum MetadataItem<'a, 'metadata> {
    Contract {
        state_type_name: &'metadata str,
        state_type_details: IoTypeDetails,
        slot_type_details: IoTypeDetails,
        tmp_type_details: IoTypeDetails,
        num_methods: u8,
        decoder: MethodsMetadataDecoder<'a, 'metadata>,
    },
    Trait {
        trait_name: &'metadata str,
        num_methods: u8,
        decoder: MethodsMetadataDecoder<'a, 'metadata>,
    },
}

impl<'a, 'metadata> MetadataItem<'a, 'metadata> {
    pub fn num_methods(&self) -> u8 {
        match self {
            MetadataItem::Contract { num_methods, .. }
            | MetadataItem::Trait { num_methods, .. } => *num_methods,
        }
    }

    pub fn into_decoder(self) -> MethodsMetadataDecoder<'a, 'metadata> {
        match self {
            MetadataItem::Contract { decoder, .. } | MetadataItem::Trait { decoder, .. } => decoder,
        }
    }
}

#[derive(Debug)]
pub struct MetadataDecoder<'metadata> {
    metadata: &'metadata [u8],
    found_contract: bool,
    found_something: bool,
}

impl<'metadata> MetadataDecoder<'metadata> {
    pub fn new(metadata: &'metadata [u8]) -> Self {
        Self {
            metadata,
            found_contract: false,
            found_something: false,
        }
    }

    pub fn decode_next<'a>(
        &'a mut self,
    ) -> Option<Result<MetadataItem<'a, 'metadata>, MetadataDecodingError<'metadata>>> {
        if self.metadata.is_empty() {
            return Some(Err(MetadataDecodingError::NotEnoughMetadata));
        }

        // Decode method kind
        let Some(metadata_kind) = ContractMetadataKind::try_from_u8(self.metadata[0]) else {
            return Some(Err(MetadataDecodingError::InvalidFirstMetadataByte {
                byte: self.metadata[0],
            }));
        };
        self.metadata = &self.metadata[1..];

        self.found_something = true;

        match metadata_kind {
            ContractMetadataKind::Contract => {
                if self.found_contract {
                    return Some(Err(MetadataDecodingError::MultipleContractsFound));
                }
                self.found_contract = true;

                Some(self.decode_contract())
            }
            ContractMetadataKind::Trait => Some(self.decode_trait()),
            // The rest are methods or arguments and can't appear here
            ContractMetadataKind::Init
            | ContractMetadataKind::UpdateStateless
            | ContractMetadataKind::UpdateStatefulRo
            | ContractMetadataKind::UpdateStatefulRw
            | ContractMetadataKind::ViewStateless
            | ContractMetadataKind::ViewStateful
            | ContractMetadataKind::EnvRo
            | ContractMetadataKind::EnvRw
            | ContractMetadataKind::TmpRo
            | ContractMetadataKind::TmpRw
            | ContractMetadataKind::SlotRo
            | ContractMetadataKind::SlotRw
            | ContractMetadataKind::Input
            | ContractMetadataKind::Output => {
                Some(Err(MetadataDecodingError::ExpectedContractOrTrait {
                    metadata_kind,
                }))
            }
        }
    }

    fn decode_contract<'a>(
        &'a mut self,
    ) -> Result<MetadataItem<'a, 'metadata>, MetadataDecodingError<'metadata>> {
        // Decode state type name without moving metadata cursor
        let state_type_name = IoTypeMetadataKind::type_name(self.metadata)
            .ok_or(MetadataDecodingError::FailedToDecodeStateTypeName)?;

        // Decode recommended capacity of the state type
        let state_type_details;
        (state_type_details, self.metadata) = IoTypeMetadataKind::type_details(self.metadata)
            .ok_or(MetadataDecodingError::InvalidStateIoType)?;

        // Decode recommended capacity of the `#[slot]` type
        let slot_type_details;
        (slot_type_details, self.metadata) = IoTypeMetadataKind::type_details(self.metadata)
            .ok_or(MetadataDecodingError::InvalidStateIoType)?;

        // Decode recommended capacity of the `#[tmp]` type
        let tmp_type_details;
        (tmp_type_details, self.metadata) = IoTypeMetadataKind::type_details(self.metadata)
            .ok_or(MetadataDecodingError::InvalidStateIoType)?;

        // Decode the number of methods
        let num_methods = self.metadata[0];
        self.metadata = &self.metadata[1..];

        Ok(MetadataItem::Contract {
            state_type_name,
            state_type_details,
            slot_type_details,
            tmp_type_details,
            num_methods,
            decoder: MethodsMetadataDecoder::new(
                &mut self.metadata,
                MethodsContainerKind::Contract,
                num_methods,
            ),
        })
    }

    fn decode_trait<'a>(
        &'a mut self,
    ) -> Result<MetadataItem<'a, 'metadata>, MetadataDecodingError<'metadata>> {
        // Decode trait name
        let trait_name_length = usize::from(self.metadata[0]);
        self.metadata = &self.metadata[1..];

        // +1 for number of arguments
        if self.metadata.len() < trait_name_length + 1 {
            return Err(MetadataDecodingError::NotEnoughMetadata);
        }

        let trait_name = &self.metadata[..trait_name_length];
        self.metadata = &self.metadata[trait_name_length..];
        let trait_name = str::from_utf8(trait_name)
            .map_err(|error| MetadataDecodingError::InvalidTraitName { trait_name, error })?;

        // Decode the number of methods
        let num_methods = self.metadata[0];
        self.metadata = &self.metadata[1..];

        Ok(MetadataItem::Trait {
            trait_name,
            num_methods,
            decoder: MethodsMetadataDecoder::new(
                &mut self.metadata,
                MethodsContainerKind::Trait,
                num_methods,
            ),
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub enum MethodsContainerKind {
    Contract,
    Trait,
    Unknown,
}

#[derive(Debug)]
pub struct MethodsMetadataDecoder<'a, 'metadata> {
    metadata: &'a mut &'metadata [u8],
    container_kind: MethodsContainerKind,
    remaining: u8,
}

impl<'a, 'metadata> MethodsMetadataDecoder<'a, 'metadata> {
    fn new(
        metadata: &'a mut &'metadata [u8],
        container_kind: MethodsContainerKind,
        num_methods: u8,
    ) -> Self {
        Self {
            metadata,
            container_kind,
            remaining: num_methods,
        }
    }

    pub fn decode_next<'b>(&'b mut self) -> Option<MethodMetadataDecoder<'b, 'metadata>> {
        if self.remaining == 0 {
            return None;
        }

        self.remaining -= 1;

        Some(MethodMetadataDecoder::new(
            self.metadata,
            self.container_kind,
        ))
    }
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
    /// Corresponds to [`ContractMetadataKind::ViewStateful`]
    ViewStateful,
}

impl MethodKind {
    pub fn has_self(&self) -> bool {
        match self {
            MethodKind::Init | MethodKind::UpdateStateless | MethodKind::ViewStateless => false,
            MethodKind::UpdateStatefulRo
            | MethodKind::UpdateStatefulRw
            | MethodKind::ViewStateful => true,
        }
    }
}

#[derive(Debug)]
pub struct MethodMetadataItem<'metadata> {
    pub method_name: &'metadata str,
    pub method_kind: MethodKind,
    pub num_arguments: u8,
}

// TODO: Would be nice to also collect fingerprint at the end
#[derive(Debug)]
pub struct MethodMetadataDecoder<'a, 'metadata> {
    metadata: &'a mut &'metadata [u8],
    container_kind: MethodsContainerKind,
}

impl<'a, 'metadata> MethodMetadataDecoder<'a, 'metadata> {
    pub fn new(metadata: &'a mut &'metadata [u8], container_kind: MethodsContainerKind) -> Self {
        Self {
            metadata,
            container_kind,
        }
    }

    pub fn decode_next(
        self,
    ) -> Result<
        (
            ArgumentsMetadataDecoder<'a, 'metadata>,
            MethodMetadataItem<'metadata>,
        ),
        MetadataDecodingError<'metadata>,
    > {
        if self.metadata.is_empty() {
            return Err(MetadataDecodingError::NotEnoughMetadata);
        }

        // Decode method kind
        let metadata_kind = ContractMetadataKind::try_from_u8(self.metadata[0]).ok_or(
            MetadataDecodingError::InvalidFirstMetadataByte {
                byte: self.metadata[0],
            },
        )?;
        *self.metadata = &self.metadata[1..];

        let method_kind = match metadata_kind {
            ContractMetadataKind::Init => MethodKind::Init,
            ContractMetadataKind::UpdateStateless => MethodKind::UpdateStateless,
            ContractMetadataKind::UpdateStatefulRo => MethodKind::UpdateStatefulRo,
            ContractMetadataKind::UpdateStatefulRw => MethodKind::UpdateStatefulRw,
            ContractMetadataKind::ViewStateless => MethodKind::ViewStateless,
            ContractMetadataKind::ViewStateful => MethodKind::ViewStateful,
            // The rest are not methods and can't appear here
            ContractMetadataKind::Contract
            | ContractMetadataKind::Trait
            | ContractMetadataKind::EnvRo
            | ContractMetadataKind::EnvRw
            | ContractMetadataKind::TmpRo
            | ContractMetadataKind::TmpRw
            | ContractMetadataKind::SlotRo
            | ContractMetadataKind::SlotRw
            | ContractMetadataKind::Input
            | ContractMetadataKind::Output => {
                return Err(MetadataDecodingError::ExpectedMethodKind { metadata_kind });
            }
        };

        let method_allowed = match self.container_kind {
            MethodsContainerKind::Contract | MethodsContainerKind::Unknown => match method_kind {
                MethodKind::Init
                | MethodKind::UpdateStateless
                | MethodKind::UpdateStatefulRo
                | MethodKind::UpdateStatefulRw
                | MethodKind::ViewStateless
                | MethodKind::ViewStateful => true,
            },
            MethodsContainerKind::Trait => match method_kind {
                MethodKind::Init
                | MethodKind::UpdateStatefulRo
                | MethodKind::UpdateStatefulRw
                | MethodKind::ViewStateful => false,
                MethodKind::UpdateStateless | MethodKind::ViewStateless => true,
            },
        };

        if !method_allowed {
            return Err(MetadataDecodingError::UnexpectedMethodKind {
                method_kind,
                container_kind: self.container_kind,
            });
        }

        if self.metadata.is_empty() {
            return Err(MetadataDecodingError::NotEnoughMetadata);
        }

        // Decode method name
        let method_name_length = usize::from(self.metadata[0]);
        *self.metadata = &self.metadata[1..];

        // +1 for number of arguments
        if self.metadata.len() < method_name_length + 1 {
            return Err(MetadataDecodingError::NotEnoughMetadata);
        }

        let method_name = &self.metadata[..method_name_length];
        *self.metadata = &self.metadata[method_name_length..];
        let method_name = str::from_utf8(method_name)
            .map_err(|error| MetadataDecodingError::InvalidMethodName { method_name, error })?;

        // Decode the number of arguments
        let num_arguments = self.metadata[0];
        *self.metadata = &self.metadata[1..];

        let decoder = ArgumentsMetadataDecoder {
            metadata: self.metadata,
            method_kind,
            remaining: num_arguments,
        };
        let item = MethodMetadataItem {
            method_name,
            method_kind,
            num_arguments,
        };

        Ok((decoder, item))
    }
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
    /// Corresponds to [`ContractMetadataKind::SlotRo`]
    SlotRo,
    /// Corresponds to [`ContractMetadataKind::SlotRw`]
    SlotRw,
    /// Corresponds to [`ContractMetadataKind::Input`]
    Input,
    /// Corresponds to [`ContractMetadataKind::Output`]
    Output,
}

#[derive(Debug)]
pub struct ArgumentMetadataItem<'metadata> {
    pub argument_name: &'metadata str,
    pub argument_kind: ArgumentKind,
    /// Exceptions:
    /// * `None` for `#[env]`
    /// * `None` for the last `#[output]` or return type otherwise in `#[init]` (see
    ///   [`ContractMetadataKind::Init`] for details)
    pub type_details: Option<IoTypeDetails>,
}

#[derive(Debug)]
pub struct ArgumentsMetadataDecoder<'a, 'metadata> {
    metadata: &'a mut &'metadata [u8],
    method_kind: MethodKind,
    remaining: u8,
}

impl<'metadata> ArgumentsMetadataDecoder<'_, 'metadata> {
    pub fn decode_next<'a>(
        &'a mut self,
    ) -> Option<Result<ArgumentMetadataItem<'metadata>, MetadataDecodingError<'metadata>>> {
        if self.remaining == 0 {
            return None;
        }

        self.remaining -= 1;

        Some(self.decode_argument())
    }

    fn decode_argument<'a>(
        &'a mut self,
    ) -> Result<ArgumentMetadataItem<'metadata>, MetadataDecodingError<'metadata>> {
        if self.metadata.is_empty() {
            return Err(MetadataDecodingError::NotEnoughMetadata);
        }

        // Decode method kind
        let metadata_kind = ContractMetadataKind::try_from_u8(self.metadata[0]).ok_or(
            MetadataDecodingError::InvalidFirstMetadataByte {
                byte: self.metadata[0],
            },
        )?;
        *self.metadata = &self.metadata[1..];

        let argument_kind = match metadata_kind {
            ContractMetadataKind::EnvRo => ArgumentKind::EnvRo,
            ContractMetadataKind::EnvRw => ArgumentKind::EnvRw,
            ContractMetadataKind::TmpRo => ArgumentKind::TmpRo,
            ContractMetadataKind::TmpRw => ArgumentKind::TmpRw,
            ContractMetadataKind::SlotRo => ArgumentKind::SlotRo,
            ContractMetadataKind::SlotRw => ArgumentKind::SlotRw,
            ContractMetadataKind::Input => ArgumentKind::Input,
            ContractMetadataKind::Output => ArgumentKind::Output,
            // The rest are not arguments and can't appear here
            ContractMetadataKind::Contract
            | ContractMetadataKind::Trait
            | ContractMetadataKind::Init
            | ContractMetadataKind::UpdateStateless
            | ContractMetadataKind::UpdateStatefulRo
            | ContractMetadataKind::UpdateStatefulRw
            | ContractMetadataKind::ViewStateless
            | ContractMetadataKind::ViewStateful => {
                return Err(MetadataDecodingError::ExpectedArgumentKind { metadata_kind });
            }
        };

        // TODO: Validate correctness of arguments order
        let argument_allowed = match self.method_kind {
            MethodKind::Init
            | MethodKind::UpdateStateless
            | MethodKind::UpdateStatefulRo
            | MethodKind::UpdateStatefulRw => match argument_kind {
                ArgumentKind::EnvRo
                | ArgumentKind::EnvRw
                | ArgumentKind::TmpRo
                | ArgumentKind::TmpRw
                | ArgumentKind::SlotRo
                | ArgumentKind::SlotRw
                | ArgumentKind::Input
                | ArgumentKind::Output => true,
            },
            MethodKind::ViewStateless | MethodKind::ViewStateful => match argument_kind {
                ArgumentKind::EnvRo
                | ArgumentKind::SlotRo
                | ArgumentKind::Input
                | ArgumentKind::Output => true,
                ArgumentKind::EnvRw
                | ArgumentKind::TmpRo
                | ArgumentKind::TmpRw
                | ArgumentKind::SlotRw => false,
            },
        };

        if !argument_allowed {
            return Err(MetadataDecodingError::UnexpectedArgumentKind {
                argument_kind,
                method_kind: self.method_kind,
            });
        }

        let (argument_name, type_details) = match argument_kind {
            ArgumentKind::EnvRo | ArgumentKind::EnvRw => ("env", None),
            ArgumentKind::TmpRo
            | ArgumentKind::TmpRw
            | ArgumentKind::SlotRo
            | ArgumentKind::SlotRw
            | ArgumentKind::Input
            | ArgumentKind::Output => {
                if self.metadata.is_empty() {
                    return Err(MetadataDecodingError::NotEnoughMetadata);
                }

                // Decode argument name
                let argument_name_length = usize::from(self.metadata[0]);
                *self.metadata = &self.metadata[1..];

                // +1 for number of arguments
                if self.metadata.len() < argument_name_length {
                    return Err(MetadataDecodingError::NotEnoughMetadata);
                }

                let argument_name = &self.metadata[..argument_name_length];
                *self.metadata = &self.metadata[argument_name_length..];
                let argument_name = str::from_utf8(argument_name).map_err(|error| {
                    MetadataDecodingError::InvalidArgumentName {
                        argument_name,
                        error,
                    }
                })?;

                let recommended_capacity = match argument_kind {
                    ArgumentKind::EnvRo
                    | ArgumentKind::EnvRw
                    | ArgumentKind::TmpRo
                    | ArgumentKind::TmpRw
                    | ArgumentKind::SlotRo
                    | ArgumentKind::SlotRw => None,
                    ArgumentKind::Input => {
                        let recommended_capacity;
                        (recommended_capacity, *self.metadata) =
                            IoTypeMetadataKind::type_details(self.metadata).ok_or(
                                MetadataDecodingError::InvalidArgumentIoType {
                                    argument_name,
                                    argument_kind,
                                },
                            )?;

                        Some(recommended_capacity)
                    }
                    ArgumentKind::Output => {
                        let last_argument = self.remaining == 0;
                        // May be skipped for `#[init]`, see `ContractMetadataKind::Init` for details
                        if matches!((self.method_kind, last_argument), (MethodKind::Init, true)) {
                            None
                        } else {
                            let recommended_capacity;
                            (recommended_capacity, *self.metadata) =
                                IoTypeMetadataKind::type_details(self.metadata).ok_or(
                                    MetadataDecodingError::InvalidArgumentIoType {
                                        argument_name,
                                        argument_kind,
                                    },
                                )?;

                            Some(recommended_capacity)
                        }
                    }
                };

                (argument_name, recommended_capacity)
            }
        };

        Ok(ArgumentMetadataItem {
            argument_name,
            argument_kind,
            type_details,
        })
    }
}
