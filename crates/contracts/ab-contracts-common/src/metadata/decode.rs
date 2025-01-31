use crate::metadata::ContractMetadataKind;
use ab_contracts_io_type::metadata::IoTypeMetadataKind;
use core::str;
use core::str::Utf8Error;

#[derive(Debug, thiserror::Error)]
pub enum MetadataDecodingError<'metadata> {
    #[error("Not enough metadata to decode")]
    NotEnoughMetadata,
    #[error("Invalid first metadata byte")]
    InvalidFirstMetadataByte { byte: u8 },
    #[error("Multiple contracts found")]
    MultipleContractsFound,
    #[error("Expected contract or trait kind, found something else: {metadata_kind:?}")]
    ExpectedContractOrTrait { metadata_kind: ContractMetadataKind },
    #[error("Invalid state type name {state_type_name:?}: {error}")]
    InvalidStateTypeName {
        state_type_name: &'metadata [u8],
        error: Utf8Error,
    },
    #[error("Invalid state I/O type")]
    InvalidStateIoType,
    #[error("Invalid trait name {trait_name:?}: {error}")]
    InvalidTraitName {
        trait_name: &'metadata [u8],
        error: Utf8Error,
    },
    #[error("Unexpected method kind {method_kind:?} for container kind {container_kind:?}")]
    UnexpectedMethodKind {
        method_kind: MethodKind,
        container_kind: MethodsContainerKind,
    },
    #[error("Expected method kind, found something else: {metadata_kind:?}")]
    ExpectedMethodKind { metadata_kind: ContractMetadataKind },
    #[error("Invalid method name {method_name:?}: {error}")]
    InvalidMethodName {
        method_name: &'metadata [u8],
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
        argument_name: &'metadata [u8],
        error: Utf8Error,
    },
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
        recommended_state_capacity: u32,
        recommended_slot_capacity: u32,
        recommended_tmp_capacity: u32,
        decoder: MethodsMetadataDecoder<'a, 'metadata>,
    },
    Trait {
        trait_name: &'metadata str,
        decoder: MethodsMetadataDecoder<'a, 'metadata>,
    },
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
            | ContractMetadataKind::ViewStatefulRo
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
        let state_type_name = {
            let state_type_name_length = usize::from(self.metadata[0]);

            if self.metadata.len() < 1 + state_type_name_length {
                return Err(MetadataDecodingError::NotEnoughMetadata);
            }

            let state_type_name = &self.metadata[1..][..state_type_name_length];
            str::from_utf8(state_type_name).map_err(|error| {
                MetadataDecodingError::InvalidStateTypeName {
                    state_type_name,
                    error,
                }
            })?
        };

        // Decode recommended capacity of the state type
        let recommended_state_capacity;
        (recommended_state_capacity, self.metadata) =
            IoTypeMetadataKind::recommended_capacity(self.metadata)
                .ok_or(MetadataDecodingError::InvalidStateIoType)?;

        // Decode recommended capacity of the `#[slot]` type
        let recommended_slot_capacity;
        (recommended_slot_capacity, self.metadata) =
            IoTypeMetadataKind::recommended_capacity(self.metadata)
                .ok_or(MetadataDecodingError::InvalidStateIoType)?;

        // Decode recommended capacity of the `#[tmp]` type
        let recommended_tmp_capacity;
        (recommended_tmp_capacity, self.metadata) =
            IoTypeMetadataKind::recommended_capacity(self.metadata)
                .ok_or(MetadataDecodingError::InvalidStateIoType)?;

        // Decode the number of methods
        let num_methods = self.metadata[0];
        self.metadata = &self.metadata[1..];

        Ok(MetadataItem::Contract {
            state_type_name,
            recommended_state_capacity,
            recommended_slot_capacity,
            recommended_tmp_capacity,
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
    /// Corresponds to [`ContractMetadataKind::ViewStatefulRo`]
    ViewStatefulRo,
}

impl MethodKind {
    pub fn has_self(&self) -> bool {
        match self {
            MethodKind::Init | MethodKind::UpdateStateless | MethodKind::ViewStateless => false,
            MethodKind::UpdateStatefulRo
            | MethodKind::UpdateStatefulRw
            | MethodKind::ViewStatefulRo => true,
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
                | MethodKind::ViewStatefulRo => true,
            },
            MethodsContainerKind::Trait => match method_kind {
                MethodKind::Init
                | MethodKind::UpdateStatefulRo
                | MethodKind::UpdateStatefulRw
                | MethodKind::ViewStatefulRo => false,
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

#[derive(Debug)]
pub struct ArgumentMetadataItem<'metadata> {
    pub argument_name: &'metadata str,
    pub argument_kind: ArgumentKind,
    /// Exceptions:
    /// * `None` for `#[env]`
    /// * `None` for `#[result]` in `#[init]`
    pub recommended_capacity: Option<u32>,
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
            return Err(MetadataDecodingError::UnexpectedArgumentKind {
                argument_kind,
                method_kind: self.method_kind,
            });
        }

        let (argument_name, recommended_capacity) = match argument_kind {
            ArgumentKind::EnvRo | ArgumentKind::EnvRw => ("env", None),
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

                let recommended_capacity = if matches!(
                    (self.method_kind, argument_kind),
                    (MethodKind::Init, ArgumentKind::Result)
                ) {
                    None
                } else {
                    let recommended_capacity;
                    (recommended_capacity, *self.metadata) =
                        IoTypeMetadataKind::recommended_capacity(self.metadata).ok_or(
                            MetadataDecodingError::InvalidArgumentIoType {
                                argument_name,
                                argument_kind,
                            },
                        )?;

                    Some(recommended_capacity)
                };

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
