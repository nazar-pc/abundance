use crate::build::enum_impl::enum_name_from_impl;
use std::collections::hash_map::OccupiedError;
use std::collections::{HashMap, HashSet};
use std::mem;
use std::path::Path;
use std::rc::Rc;
use syn::{Ident, ItemEnum, ItemImpl, Variant};

#[derive(Debug)]
pub(super) struct KnownEnumDefinition {
    pub(super) own_instructions: Vec<Rc<Variant>>,
    pub(super) instructions: Vec<Rc<Variant>>,
    pub(super) ignored_instructions: Rc<HashSet<Ident>>,
    pub(super) direct_dependencies: Rc<[Ident]>,
    pub(super) dependencies_for_enablement: HashSet<Rc<[Ident]>>,
    pub(super) source: Rc<Path>,
}

#[derive(Debug)]
pub(super) struct PendingEnumDefinition {
    pub(super) original_item_enum: ItemEnum,
}

#[derive(Debug)]
pub(super) struct KnownOriginalEnumDecodingImpl {
    pub(super) item_impl: ItemImpl,
    pub(super) source: Rc<Path>,
}

#[derive(Debug)]
pub(super) struct PendingEnumImpl {
    pub(super) item_impl: ItemImpl,
}

#[derive(Debug)]
pub(super) struct PendingEnumDisplayImpl {
    pub(super) item_impl: ItemImpl,
}

#[derive(Debug)]
pub(super) struct PendingEnumOperandsImpl {
    pub(super) item_impl: ItemImpl,
}

#[derive(Debug)]
pub(super) struct KnownEnumCsrImpl {
    pub(super) item_impl: ItemImpl,
    pub(super) source: Rc<Path>,
}

#[derive(Debug)]
pub(super) struct PendingEnumCsrImpl {
    pub(super) item_impl: ItemImpl,
}

#[derive(Debug)]
pub(super) struct KnownEnumExecutionImpl {
    pub(super) item_impl: ItemImpl,
    pub(super) source: Rc<Path>,
}

#[derive(Debug)]
pub(super) struct PendingEnumExecutionImpl {
    pub(super) item_impl: ItemImpl,
}

pub(super) struct State {
    known_enum_definitions: HashMap<Ident, KnownEnumDefinition>,
    pending_enum_definitions: Vec<PendingEnumDefinition>,
    known_original_enum_decoding_impls: HashMap<Ident, KnownOriginalEnumDecodingImpl>,
    pending_enum_impls: Vec<PendingEnumImpl>,
    pending_enum_display_impls: Vec<PendingEnumDisplayImpl>,
    pending_enum_operands_impls: Vec<PendingEnumOperandsImpl>,
    known_enum_csr_impls: HashMap<Ident, KnownEnumCsrImpl>,
    pending_enum_csr_impls: Vec<PendingEnumCsrImpl>,
    known_enum_execution_impls: HashMap<Ident, KnownEnumExecutionImpl>,
    pending_enum_execution_impls: Vec<PendingEnumExecutionImpl>,
}

impl State {
    pub(super) fn new() -> Self {
        Self {
            known_enum_definitions: HashMap::new(),
            pending_enum_definitions: Vec::new(),
            known_original_enum_decoding_impls: HashMap::new(),
            pending_enum_impls: Vec::new(),
            pending_enum_display_impls: Vec::new(),
            pending_enum_operands_impls: Vec::new(),
            known_enum_csr_impls: HashMap::new(),
            pending_enum_csr_impls: Vec::new(),
            known_enum_execution_impls: HashMap::new(),
            pending_enum_execution_impls: Vec::new(),
        }
    }

    pub(super) fn get_known_enum_definition(
        &self,
        enum_name: &Ident,
    ) -> Option<&KnownEnumDefinition> {
        self.known_enum_definitions.get(enum_name)
    }

    pub(super) fn get_known_original_enum_decoding_impl(
        &self,
        enum_name: &Ident,
    ) -> Option<&KnownOriginalEnumDecodingImpl> {
        self.known_original_enum_decoding_impls.get(enum_name)
    }

    pub(super) fn get_known_enum_csr_impl(&self, enum_name: &Ident) -> Option<&KnownEnumCsrImpl> {
        self.known_enum_csr_impls.get(enum_name)
    }

    pub(super) fn get_known_enum_execution_impl(
        &self,
        enum_name: &Ident,
    ) -> Option<&KnownEnumExecutionImpl> {
        self.known_enum_execution_impls.get(enum_name)
    }

    pub(super) fn insert_known_enum_definition(
        &mut self,
        original_item_enum: ItemEnum,
        item_enum: ItemEnum,
        ignored_instructions: HashSet<Ident>,
        direct_dependencies: Rc<[Ident]>,
        dependencies_for_enablement: HashSet<Rc<[Ident]>>,
        source: Rc<Path>,
    ) -> anyhow::Result<()> {
        let known_enum_definition = KnownEnumDefinition {
            own_instructions: original_item_enum
                .variants
                .into_iter()
                .map(Rc::new)
                .collect(),
            instructions: item_enum.variants.into_iter().map(Rc::new).collect(),
            ignored_instructions: Rc::new(ignored_instructions),
            direct_dependencies,
            dependencies_for_enablement,
            source,
        };
        if let Err(OccupiedError { entry, value }) = self
            .known_enum_definitions
            .try_insert(original_item_enum.ident.clone(), known_enum_definition)
            && entry.get().own_instructions != value.own_instructions
        {
            return Err(anyhow::anyhow!(
                "Instruction enum `{}` is already defined in `{}`, a different duplicate found in \
                `{}`",
                original_item_enum.ident,
                entry.get().source.display(),
                value.source.display(),
            ));
        }

        Ok(())
    }

    pub(super) fn insert_known_original_enum_decoding_impl(
        &mut self,
        item_impl: ItemImpl,
        source: Rc<Path>,
    ) -> anyhow::Result<()> {
        let enum_name = enum_name_from_impl(&item_impl);

        if let Err(OccupiedError { entry, value }) =
            self.known_original_enum_decoding_impls.try_insert(
                enum_name.clone(),
                KnownOriginalEnumDecodingImpl {
                    item_impl: item_impl.clone(),
                    source: Rc::clone(&source),
                },
            )
            && entry.get().item_impl != value.item_impl
        {
            return Err(anyhow::anyhow!(
                "Implementation for enum `{}` is already defined in `{}`, a different duplicate \
                found in `{}`\n{:?}\n{:?}",
                enum_name,
                entry.get().source.display(),
                source.display(),
                entry.get().item_impl,
                item_impl,
            ));
        }

        Ok(())
    }

    pub(super) fn insert_known_enum_csr_impl(
        &mut self,
        item_impl: ItemImpl,
        source: Rc<Path>,
    ) -> anyhow::Result<()> {
        let enum_name = enum_name_from_impl(&item_impl);

        if let Err(OccupiedError { entry, value }) = self.known_enum_csr_impls.try_insert(
            enum_name.clone(),
            KnownEnumCsrImpl {
                item_impl,
                source: Rc::clone(&source),
            },
        ) && entry.get().item_impl != value.item_impl
        {
            return Err(anyhow::anyhow!(
                "Execution CSR implementation for enum `{}` is already defined in `{}`, a \
                different duplicate found in `{}`",
                enum_name,
                entry.get().source.display(),
                source.display(),
            ));
        }

        Ok(())
    }

    pub(super) fn insert_known_enum_execution_impl(
        &mut self,
        item_impl: ItemImpl,
        source: Rc<Path>,
    ) -> anyhow::Result<()> {
        let enum_name = enum_name_from_impl(&item_impl);

        if let Err(OccupiedError { entry, value }) = self.known_enum_execution_impls.try_insert(
            enum_name.clone(),
            KnownEnumExecutionImpl {
                item_impl,
                source: Rc::clone(&source),
            },
        ) && entry.get().item_impl != value.item_impl
        {
            return Err(anyhow::anyhow!(
                "Execution implementation for enum `{}` is already defined in `{}`, a different \
                duplicate found in `{}`",
                enum_name,
                entry.get().source.display(),
                source.display(),
            ));
        }

        Ok(())
    }

    pub(super) fn add_pending_enum_definition(
        &mut self,
        pending_enum_definition: PendingEnumDefinition,
    ) {
        self.pending_enum_definitions.push(pending_enum_definition);
    }

    pub(super) fn take_pending_enum_definitions(&mut self) -> Vec<PendingEnumDefinition> {
        mem::take(&mut self.pending_enum_definitions)
    }

    pub(super) fn add_pending_enum_impl(&mut self, pending_enum_impl: PendingEnumImpl) {
        self.pending_enum_impls.push(pending_enum_impl);
    }

    pub(super) fn take_pending_enum_display_impls(&mut self) -> Vec<PendingEnumDisplayImpl> {
        mem::take(&mut self.pending_enum_display_impls)
    }

    pub(super) fn add_pending_enum_display_impl(
        &mut self,
        pending_enum_display_impl: PendingEnumDisplayImpl,
    ) {
        self.pending_enum_display_impls
            .push(pending_enum_display_impl);
    }

    pub(super) fn take_pending_enum_impls(&mut self) -> Vec<PendingEnumImpl> {
        mem::take(&mut self.pending_enum_impls)
    }

    pub(super) fn add_pending_enum_operands_impl(
        &mut self,
        pending_enum_operands_impl: PendingEnumOperandsImpl,
    ) {
        self.pending_enum_operands_impls
            .push(pending_enum_operands_impl);
    }

    pub(super) fn take_pending_enum_operands_impls(&mut self) -> Vec<PendingEnumOperandsImpl> {
        mem::take(&mut self.pending_enum_operands_impls)
    }

    pub(super) fn add_pending_enum_csr_impl(&mut self, pending_enum_csr_impl: PendingEnumCsrImpl) {
        self.pending_enum_csr_impls.push(pending_enum_csr_impl);
    }

    pub(super) fn take_pending_enum_csr_impls(&mut self) -> Vec<PendingEnumCsrImpl> {
        mem::take(&mut self.pending_enum_csr_impls)
    }

    pub(super) fn add_pending_enum_execution_impl(
        &mut self,
        pending_enum_execution_impl: PendingEnumExecutionImpl,
    ) {
        self.pending_enum_execution_impls
            .push(pending_enum_execution_impl);
    }

    pub(super) fn take_pending_enum_execution_impls(&mut self) -> Vec<PendingEnumExecutionImpl> {
        mem::take(&mut self.pending_enum_execution_impls)
    }
}
