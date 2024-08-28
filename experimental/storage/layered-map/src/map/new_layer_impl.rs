// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    flatten_perfect_tree::{FlattenPerfectTree, FptRef, FptRefMut},
    map::new_layer_impl::OutputPositionInfo::BelowPeak,
    metrics::TIMER,
    node::{CollisionCell, LeafContent, LeafNode, NodeRef, NodeRef::Empty, NodeStrongRef},
    Key, KeyHash, LayeredMap, MapLayer, Value,
};
use aptos_drop_helper::ArcAsyncDrop;
use aptos_metrics_core::TimerHelper;
use itertools::Itertools;
use std::collections::BTreeMap;

impl<K, V, S> LayeredMap<K, V, S>
where
    K: ArcAsyncDrop + Key,
    V: ArcAsyncDrop + Value,
{
    fn merge_up(&self, left: NodeRef<K, V>, right: NodeRef<K, V>) -> NodeRef<K, V> {
        use crate::node::NodeRef::*;

        match (&left, &right) {
            (Empty, Leaf(..)) => right,
            (Leaf(..), Empty) => left,
            (Empty, Empty) => unreachable!("merge_up with two empty nodes"),
            _ => self.new_internal(left, right),
        }
    }

    fn create_tree(
        &self,
        depth: usize,
        current_root: FptRef<K, V>,
        items: &[Item<K, V>],
    ) -> Builder<'a, K, V> {
        if items.is_empty() {
            return current_root.weak_ref();
        }

        // See if the whole range is of the same key hash, which maps to a leaf node
        let first_key_hash = items[0].key_hash();
        if first_key_hash == items[items.len() - 1].key_hash() {
            match &current_root {
                NodeStrongRef::Empty => return self.new_leaf(first_key_hash, items),
                NodeStrongRef::Leaf(leaf) => {
                    if leaf.key_hash == first_key_hash {
                        return self.new_leaf_overwriting_old(first_key_hash, leaf, items);
                    }
                },
                NodeStrongRef::Internal(_) => {},
            }
        }

        let pivot = items.partition_point(|item| !item.key_hash.bit(depth));
        let (left_items, right_items) = items.split_at(pivot);
        let (left_root, right_root) = self.branch_down(depth, current_root);
        self.merge_up(
            self.create_tree(depth + 1, left_root, left_items),
            self.create_tree(depth + 1, right_root, right_items),
        )
    }

    pub fn new_layer_with_hasher(&self, kvs: &[(K, V)], hash_builder: &S) -> MapLayer<K, V>
    where
        S: core::hash::BuildHasher,
    {
        let _timer = TIMER.timer_with(&[self.top_layer.use_case(), "new_layer"]);

        // Hash the keys and sort items in key hash order.
        //
        // n.b. no need to dedup at this point, as we will do it anyway at the leaf level.
        let items = kvs
            .iter()
            .map(|kv| {
                let key = &kv.0;
                let key_hash = KeyHash(hash_builder.hash_one(key));
                Item { key_hash, kv }
            })
            .sorted_by_key(Item::full_key)
            .collect_vec();

        let mut new_peak = FlattenPerfectTree::new_empty(10);
        let builder = SubTreeBuilder {
            map: self,
            depth: 0,
            position_info: self.peak().root(),
            position_in_new_peak: new_peak.root_mut(),
            items: &items,
        };
        builder.build().finalize();

        self.top_layer.spawn(new_peak, self.base_layer())
    }

    pub fn new_layer(&self, items: &[(K, V)]) -> MapLayer<K, V>
    where
        S: core::hash::BuildHasher + Default,
    {
        self.new_layer_with_hasher(items, &Default::default())
    }
}

pub(crate) struct Item<'a, K, V> {
    key_hash: KeyHash,
    kv: &'a (K, V),
}

impl<'a, K, V> Item<'a, K, V> {
    fn key_hash(&self) -> KeyHash {
        self.key_hash
    }

    fn key(&self) -> &'a K {
        &self.kv.0
    }

    /// Full key used for sorting and deduplication.
    ///
    /// Inequality is detected if key hash is different, keys only need to be compared in case of
    /// hash collision.
    fn full_key(&self) -> (KeyHash, &'a K) {
        (self.key_hash(), self.key())
    }

    fn kv(&self) -> &(K, V) {
        self.kv
    }
}

fn to_leaf_content<K: Key, V: Value>(items: &[Item<K, V>], layer: u64) -> LeafContent<K, V> {
    assert!(!items.is_empty());
    if items.len() == 1 {
        let (key, value) = items[0].kv().clone();
        LeafContent::UniqueLatest { key, value }
    } else {
        // deduplication
        let mut map: BTreeMap<_, _> = items
            .iter()
            .map(|item| {
                let (key, value) = item.kv().clone();
                (key, CollisionCell { value, layer })
            })
            .collect();
        if map.len() == 1 {
            let (key, cell) = map.pop_first().unwrap();
            LeafContent::UniqueLatest {
                key,
                value: cell.value,
            }
        } else {
            LeafContent::Collision(map)
        }
    }
}

enum PositionInfo<'a, K, V> {
    AbovePeakFeet(FptRef<'a, K, V>),
    PeakFootOrBelow(NodeStrongRef<K, V>),
}

impl<'a, K, V> PositionInfo<'a, K, V> {
    fn is_in_peak(&self) -> bool {
        matches!(self, Self::AbovePeakFeet(..))
    }

    fn expect_peak_foot_or_below(&self) -> NodeStrongRef<K, V> {
        match self {
            Self::AbovePeakFeet(..) => panic!("Still in Peak"),
            Self::PeakFootOrBelow(node) => node.clone(),
        }
    }

    fn children(self, depth: usize, base_layer: u64) -> (Self, Self) {
        use PositionInfo::*;

        match self {
            AbovePeakFeet(fpt) => {
                let (left, right) = fpt.expect_sub_trees();
                if left.is_single_node() {
                    (
                        PeakFootOrBelow(left.expect_single_node(base_layer)),
                        PeakFootOrBelow(right.expect_single_node(base_layer)),
                    )
                } else {
                    (AbovePeakFeet(left), AbovePeakFeet(right))
                }
            },
            PeakFootOrBelow(node) => {
                let (left, right) = node.children(depth, base_layer);
                (PeakFootOrBelow(left), PeakFootOrBelow(right))
            },
        }
    }
}

enum PendingBuild<'a, K, V> {
    AbovePeakFeet,
    FootOfPeak(&'a mut NodeRef<K, V>),
    BelowPeak,
}

impl<'a, K, V> PendingBuild<'a, K, V> {
    fn seal_with_node(&mut self, node: NodeRef<K, V>) -> BuiltSubTree<K, V> {
        match self {
            PendingBuild::AbovePeakFeet => unreachable!("Trying to put node above peak feet."),
            PendingBuild::FootOfPeak(ref_mut) => {
                **ref_mut = node;
                BuiltSubTree::InOrAtFootOfPeak
            },
            PendingBuild::BelowPeak => BuiltSubTree::BelowPeak(node),
        }
    }

    fn seal_with_children(
        &mut self,
        left: BuiltSubTree<K, V>,
        right: BuiltSubTree<K, V>,
        layer: u64,
    ) -> BuiltSubTree<K, V> {
        match (left, right) {
            (BuiltSubTree::InOrAtFootOfPeak, BuiltSubTree::InOrAtFootOfPeak) => {
                assert!(
                    matches!(self, PendingBuild::AbovePeakFeet),
                    "Expecting nodes."
                );
                BuiltSubTree::InOrAtFootOfPeak
            },
            (BuiltSubTree::BelowPeak(left), BuiltSubTree::BelowPeak(right)) => {
                let internal_node = Self::make_parent(left, right, layer);
                self.seal_with_node(internal_node)
            },
            _ => unreachable!("Children should be of same flavor."),
        }
    }

    fn make_parent(left: NodeRef<K, V>, right: NodeRef<K, V>, layer: u64) -> NodeRef<K, V> {
        use crate::node::NodeRef::*;

        match (&left, &right) {
            (Empty, Leaf(..)) => right,
            (Leaf(..), Empty) => left,
            (Empty, Empty) => Empty,
            _ => NodeRef::new_internal(left, right, layer),
        }
    }
}

#[must_use = "Must finalize()"]
enum BuiltSubTree<K, V> {
    InOrAtFootOfPeak,
    BelowPeak(NodeRef<K, V>),
}

impl<K, V> BuiltSubTree<K, V> {
    fn finalize(self) {
        // note: need to carry height to assert more strongly
        // (that it's built all the way to the root)
        assert!(
            matches!(self, BuiltSubTree::InOrAtFootOfPeak),
            "Haven't reached the peak."
        );
    }
}

enum MaybeEndRecursion<'a, K, V> {
    Continue(SubTreeBuilder<'a, K, V>),
    End(NodeRef<K, V>),
}

struct SubTreeBuilder<'a, K, V> {
    map: &'a LayeredMap<K, V>,
    depth: usize,
    position_info: PositionInfo<'a, K, V>,
    position_in_new_peak: Option<FptRefMut<'a, K, V>>,
    items: &'a [Item<'a, K, V>],
}

impl<'a, K, V> SubTreeBuilder<'a, K, V> {
    pub fn build(mut self) -> BuiltSubTree<K, V> {
        use MaybeEndRecursion::*;

        let mut pending_build = self.init_pending_build();

        match self.maybe_end_recursion() {
            Continue(myself) => {
                let layer = myself.map.top_layer() + 1;
                let (left, right) = myself.branch();
                pending_build.seal_with_children(left.build(), right.build(), layer)
            },
            End(node) => pending_build.seal_with_node(node),
        }
    }

    fn init_pending_build(&mut self) -> PendingBuild<K, V> {
        match self.position_in_new_peak.take() {
            None => PendingBuild::BelowPeak,
            Some(fpt) => {
                if fpt.is_single_node() {
                    PendingBuild::FootOfPeak(fpt.expect_into_single_node_ref())
                } else {
                    PendingBuild::AbovePeakFeet
                }
            },
        }
    }

    fn all_items_same_key_hash(&self) -> Option<KeyHash> {
        let items = &self.items;

        assert!(!items.is_empty());
        let first_key_hash = items[0].key_hash();
        if first_key_hash == items[items.len() - 1].key_hash() {
            Some(first_key_hash)
        } else {
            None
        }
    }

    fn still_in_peak(&self) -> bool {
        self.position_info.is_in_peak()
            || self
                .position_in_new_peak
                .as_ref()
                .map_or(false, |fpt| !fpt.is_single_node())
    }

    fn maybe_end_recursion(self) -> MaybeEndRecursion<'a, K, V> {
        use MaybeEndRecursion::*;

        if self.still_in_peak() {
            // Can't start building up unless deep enough to see the bottoms of the peaks.
            Continue(self)
        } else if self.items.is_empty() {
            // No new leaves to add in this branch, return weak ref to the current node.
            End(self.position_info.expect_peak_foot_or_below().weak_ref())
        } else {
            match self.all_items_same_key_hash() {
                None => {
                    // Still multiple leaves to add, branch further down.
                    Continue(self)
                },
                Some(key_hash) => {
                    // All new items belong to the same new leaf node.
                    match self.position_info.expect_peak_foot_or_below() {
                        NodeStrongRef::Empty => End(self.new_leaf(key_hash, self.items)),
                        NodeStrongRef::Leaf(leaf) => {
                            if leaf.key_hash == key_hash {
                                End(self.new_leaf_overwriting_old(key_hash, &leaf, self.items))
                            } else {
                                Continue(self)
                            }
                        },
                        NodeStrongRef::Internal(_) => Continue(self),
                    }
                }, // end Some(key_hash) == all_items_same_key_hash()
            } // end match
        } // end else
    }

    fn branch(self) -> (Self, Self) {
        let Self {
            map,
            depth,
            position_info,
            position_in_new_peak,
            items,
        } = self;

        let (left, right) = position_info.children(depth, self.map.base_layer());
        let (out_left, out_right) = position_in_new_peak.unwrap().try_into_sub_trees();

        let pivot = items.partition_point(|item| !item.key_hash.bit(depth));
        let (items_left, items_right) = items.split_at(pivot);

        let left = Self {
            map,
            depth: depth + 1,
            position_info: left,
            position_in_new_peak: out_left,
            items: items_left,
        };
        let right = Self {
            map,
            depth: depth + 1,
            position_info: right,
            position_in_new_peak: out_right,
            items: items_right,
        };
        (left, right)
    }

    fn new_leaf(&self, key_hash: KeyHash, items: &[Item<K, V>]) -> NodeRef<K, V> {
        let new_layer = self.map.top_layer() + 1;
        NodeRef::new_leaf(key_hash, to_leaf_content(items, new_layer), new_layer)
    }

    fn new_leaf_overwriting_old(
        &self,
        key_hash: KeyHash,
        old_leaf: &LeafNode<K, V>,
        new_items: &[Item<K, V>],
    ) -> NodeRef<K, V> {
        let new_layer = self.map.top_layer() + 1;

        let old = old_leaf.content.clone();
        let new = to_leaf_content(new_items, new_layer);
        let content = old.combined_with(old_leaf.layer, new, new_layer, self.map.base_layer());

        NodeRef::new_leaf(key_hash, content, new_layer)
    }
}
