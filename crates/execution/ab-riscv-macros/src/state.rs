use crate::enum_definition::InstructionDefinition;
use crate::enum_impl::enum_name_from_impl;
use std::collections::HashMap;
use std::collections::hash_map::OccupiedError;
use std::mem;
use std::path::Path;
use std::rc::Rc;
use syn::{Ident, ItemEnum, ItemImpl, Variant};

#[derive(Debug)]
pub(super) struct KnownEnumDefinition {
    pub(super) instructions: Vec<Rc<Variant>>,
    pub(super) dependencies: Vec<Ident>,
    pub(super) source: Rc<Path>,
}

#[derive(Debug)]
pub(super) struct PendingEnumDefinition {
    pub(super) instruction_definition: InstructionDefinition,
    pub(super) item_enum: ItemEnum,
}

#[derive(Debug)]
pub(super) struct KnownEnumImpl {
    pub(super) item_impl: ItemImpl,
    pub(super) source: Rc<Path>,
}

#[derive(Debug)]
pub(super) struct PendingEnumImpl {
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
    known_enum_impls: HashMap<Ident, KnownEnumImpl>,
    pending_enum_impls: Vec<PendingEnumImpl>,
    known_enum_execution_impls: HashMap<Ident, KnownEnumExecutionImpl>,
    pending_enum_execution_impls: Vec<PendingEnumExecutionImpl>,
}

impl State {
    pub(super) fn new() -> Self {
        Self {
            known_enum_definitions: HashMap::new(),
            pending_enum_definitions: Vec::new(),
            known_enum_impls: HashMap::new(),
            pending_enum_impls: Vec::new(),
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

    pub(super) fn get_known_enum_impl(&self, enum_name: &Ident) -> Option<&KnownEnumImpl> {
        self.known_enum_impls.get(enum_name)
    }

    pub(super) fn get_known_enum_execution_impl(
        &self,
        enum_name: &Ident,
    ) -> Option<&KnownEnumExecutionImpl> {
        self.known_enum_execution_impls.get(enum_name)
    }

    pub(super) fn insert_known_enum_definition(
        &mut self,
        item_enum: ItemEnum,
        dependencies: Vec<Ident>,
        source: Rc<Path>,
    ) -> anyhow::Result<()> {
        let known_enum_definition = KnownEnumDefinition {
            instructions: item_enum.variants.into_iter().map(Rc::new).collect(),
            dependencies,
            source,
        };
        if let Err(OccupiedError { entry, value }) = self
            .known_enum_definitions
            .try_insert(item_enum.ident.clone(), known_enum_definition)
            && entry.get().instructions != value.instructions
        {
            return Err(anyhow::anyhow!(
                "Instruction enum `{}` is already defined in `{}`, a different duplicate found in \
                `{}`",
                item_enum.ident,
                entry.get().source.display(),
                value.source.display(),
            ));
        }

        Ok(())
    }

    pub(super) fn insert_known_enum_impl(
        &mut self,
        item_impl: ItemImpl,
        source: Rc<Path>,
    ) -> anyhow::Result<()> {
        let enum_name = enum_name_from_impl(&item_impl);

        if let Err(OccupiedError { entry, value }) = self.known_enum_impls.try_insert(
            enum_name.clone(),
            KnownEnumImpl {
                item_impl,
                source: source.clone(),
            },
        ) && entry.get().item_impl != value.item_impl
        {
            return Err(anyhow::anyhow!(
                "Implementation for enum `{}` is already defined in `{}`, a different duplicate \
                found in `{}`",
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
                source: source.clone(),
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

    pub(super) fn take_pending_enum_impls(&mut self) -> Vec<PendingEnumImpl> {
        mem::take(&mut self.pending_enum_impls)
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
