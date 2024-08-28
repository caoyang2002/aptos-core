// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::node::{NodeRef, NodeStrongRef};

pub(crate) struct FlattenPerfectTree<K, V> {
    leaves: Vec<NodeRef<K, V>>,
    height: usize,
}

impl<K, V> FlattenPerfectTree<K, V> {
    pub fn new_empty(height: usize) -> Self {
        let num_leaves = if height == 0 { 0 } else { 1 << (height - 1) };

        let mut leaves = Vec::new();
        leaves.resize_with(num_leaves, || NodeRef::Empty);

        Self { leaves, height }
    }

    pub fn root(&self) -> FptRef<K, V> {
        FptRef {
            leaves: &self.leaves,
        }
    }

    pub fn root_mut(&mut self) -> FptRefMut<K, V> {
        FptRefMut {
            leaves: &mut self.leaves,
        }
    }
}

pub(crate) struct FptRef<'a, K, V> {
    leaves: &'a [NodeRef<K, V>],
}

impl<'a, K, V> FptRef<'a, K, V> {
    pub fn expect_sub_trees(self) -> (Self, Self) {
        todo!()
    }

    pub fn is_single_node(&self) -> bool {
        self.leaves.len() == 1
    }

    pub fn expect_single_node(&self, base_layer: u64) -> NodeStrongRef<K, V> {
        assert!(self.is_single_node());
        self.leaves[0].get_strong(base_layer)
    }
}

pub(crate) struct FptRefMut<'a, K, V> {
    leaves: &'a mut [NodeRef<K, V>],
}

impl<'a, K, V> FptRefMut<'a, K, V> {
    pub fn try_into_sub_trees(self) -> (Option<Self>, Option<Self>) {
        if self.leaves.len() == 1 {
            (None, None)
        } else {
            let (left, right) = self.leaves.split_at_mut(self.leaves.len() / 2);
            (Some(Self { leaves: left }), Some(Self { leaves: right }))
        }
    }

    pub fn is_single_node(&self) -> bool {
        self.leaves.len() == 1
    }

    pub fn expect_into_single_node_ref(self) -> &'a mut NodeRef<K, V> {
        assert!(self.is_single_node());
        &mut self.leaves[0]
    }
}
