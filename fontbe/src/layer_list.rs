//! LayerListBuilder for creating optimized COLR LayerLists with layer reuse.
//!
//! This module provides utilities for building COLR v1 LayerLists that automatically
//! deduplicate common paint subsequences, reducing table size.

use std::collections::HashMap;

use write_fonts::tables::colr::{LayerList, Paint};

/// Maximum length of subsequences to check for layer reuse.
///
/// This limit balances reuse opportunities against O(n²) search complexity.
/// Larger values would increase build time with diminishing returns.
const MAX_REUSE_LEN: usize = 32;

/// Maximum number of layers in a single PaintColrLayers (uint8 limit).
///
/// When a PaintColrLayers references more than 255 layers, it must be split
/// into an n-ary tree structure.
const MAX_PAINT_COLR_LAYER_COUNT: u8 = 255;

/// Generate all valid subsequence ranges for layer reuse.
///
/// Yields (start, end) pairs where:
/// - Length is between 2 and min(num_layers, MAX_REUSE_LEN)
/// - End is exclusive (standard Rust range convention)
fn reuse_ranges(num_layers: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..num_layers).flat_map(move |lbound| {
        let min_ubound = lbound + 2; // Minimum length: 2
        let max_ubound = (lbound + MAX_REUSE_LEN + 1).min(num_layers + 1);
        (min_ubound..max_ubound).map(move |ubound| (lbound, ubound))
    })
}

/// Cache for detecting and reusing layer subsequences.
///
/// Maintains a mapping from paint subsequences to their first occurrence
/// in the layer list, enabling deduplication by replacing duplicate sequences
/// with PaintColrLayers references.
struct LayerReuseCache {
    /// Maps paint subsequence → first layer index in LayerList
    reuse_pool: HashMap<Vec<Paint>, u32>,
}

impl LayerReuseCache {
    /// Create a new empty LayerReuseCache.
    fn new() -> Self {
        Self {
            reuse_pool: HashMap::new(),
        }
    }

    /// Attempt to find and replace reusable subsequences in the given layers.
    ///
    /// Iteratively searches for matching subsequences in the reuse pool,
    /// replacing them with PaintColrLayers references. Prioritizes longer
    /// sequences for maximum savings.
    ///
    /// Returns the modified layer list with reused subsequences replaced.
    fn try_reuse(&self, mut layers: Vec<Paint>) -> Vec<Paint> {
        loop {
            let mut found_reuse = false;

            // Generate all possible subsequence ranges, sorted by priority:
            // 1. Longer sequences first (more savings)
            // 2. Later positions (preserve earlier structure)
            // 3. Earlier starts (stable ordering)
            let mut ranges: Vec<_> = reuse_ranges(layers.len()).collect();
            ranges.sort_by_key(|(lbound, ubound)| {
                (
                    std::cmp::Reverse(ubound - lbound), // Length descending
                    std::cmp::Reverse(*ubound),         // Position descending
                    std::cmp::Reverse(*lbound),         // Start descending
                )
            });

            for (lbound, ubound) in ranges {
                let slice = &layers[lbound..ubound];

                // Check if this subsequence exists in reuse pool
                if let Some(&first_layer_index) = self.reuse_pool.get(slice) {
                    // Replace with PaintColrLayers reference
                    let num_layers = (ubound - lbound) as u8;
                    let new_paint = Paint::colr_layers(num_layers, first_layer_index);

                    layers.splice(lbound..ubound, std::iter::once(new_paint));
                    found_reuse = true;
                    break;
                }
            }

            if !found_reuse {
                break;
            }
        }

        layers
    }

    /// Register all subsequences of the given layers for future reuse.
    ///
    /// # Arguments
    /// * `layers` - The paint sequence to register
    /// * `first_index` - Starting index in the LayerList where these layers appear
    fn register(&mut self, layers: &[Paint], first_index: u32) {
        for (lbound, ubound) in reuse_ranges(layers.len()) {
            let subsequence = layers[lbound..ubound].to_vec();
            let abs_index = first_index + lbound as u32;

            // Only insert if not already present (first occurrence wins)
            self.reuse_pool.entry(subsequence).or_insert(abs_index);
        }
    }
}

/// Builder for creating optimized LayerLists with layer reuse.
///
/// This builder accumulates Paint layers and optionally deduplicates common
/// subsequences, reducing COLR table size by replacing repeated patterns with
/// PaintColrLayers references.
pub struct LayerListBuilder {
    /// Accumulated list of all paint layers
    layers: Vec<Paint>,
    /// Optional cache for detecting and reusing layer subsequences
    reuse_cache: Option<LayerReuseCache>,
}

impl LayerListBuilder {
    /// Create a new LayerListBuilder.
    ///
    /// # Arguments
    /// * `allow_layer_reuse` - If true, enables deduplication of common layer sequences
    pub fn new(allow_layer_reuse: bool) -> Self {
        Self {
            layers: Vec::new(),
            reuse_cache: if allow_layer_reuse {
                Some(LayerReuseCache::new())
            } else {
                None
            },
        }
    }

    /// Add multiple paints as layers and return a Paint that references them.
    ///
    /// This is the main method for adding layers to the builder. It automatically:
    /// 1. Tries to reuse existing layer sequences (if layer reuse is enabled)
    /// 2. Adds the layers to the layer list
    /// 3. Registers them for future reuse
    /// 4. Handles n-ary tree building for >255 layers
    ///
    /// # Arguments
    /// * `paints` - The paints to add as layers
    ///
    /// # Returns
    /// A Paint (either a single PaintColrLayers or a tree structure) that references
    /// all the input paints.
    pub fn add_paint_layers(&mut self, paints: Vec<Paint>) -> Paint {
        if paints.is_empty() {
            // Return a no-op paint (PaintColrLayers with 0 layers)
            return Paint::colr_layers(0, 0);
        }

        // If only one paint, just return it directly (no need for PaintColrLayers wrapper)
        if paints.len() == 1 {
            return paints.into_iter().next().unwrap();
        }

        // Try to reuse existing layers
        let layers = if let Some(ref cache) = self.reuse_cache {
            cache.try_reuse(paints)
        } else {
            paints
        };

        // Check for complete reuse: if try_reuse returned a single PaintColrLayers,
        // it means all input layers matched an existing sequence. Just return it.
        if layers.len() == 1 {
            if let Paint::ColrLayers(_) = &layers[0] {
                // Complete reuse - return the reference without adding anything
                return layers.into_iter().next().unwrap();
            }
        }

        // Add the base layers to our layer list
        let first_layer_index = self.layers.len() as u32;
        let num_layers = layers.len() as u32;
        self.layers.extend(layers.iter().cloned());

        // Register for future reuse (before building tree, so we register base layers)
        if let Some(ref mut cache) = self.reuse_cache {
            cache.register(&layers, first_layer_index);
        }

        // Handle n-ary tree if needed (>255 layers)
        // This adds tree reference nodes to self.layers and returns the top-level Paint
        if num_layers > MAX_PAINT_COLR_LAYER_COUNT as u32 {
            self.build_n_ary_tree_internal(first_layer_index, num_layers)
        } else {
            // Simple case: return a single PaintColrLayers reference
            if num_layers == 1 {
                // Unwrap single-layer case
                layers.into_iter().next().unwrap()
            } else {
                Paint::colr_layers(num_layers as u8, first_layer_index)
            }
        }
    }

    /// Internal method to build an n-ary tree for referencing many layers.
    ///
    /// When num_layers > MAX_PAINT_COLR_LAYER_COUNT, this creates a tree structure:
    /// 1. Create PaintColrLayers nodes for chunks of up to 255 layers each
    /// 2. Add these nodes to the layer list
    /// 3. Recursively build if we have >255 nodes
    fn build_n_ary_tree_internal(&mut self, first_layer_index: u32, num_layers: u32) -> Paint {
        if num_layers <= MAX_PAINT_COLR_LAYER_COUNT as u32 {
            // Simple case: single PaintColrLayers
            return Paint::colr_layers(num_layers as u8, first_layer_index);
        }

        // Complex case: need to split into multiple PaintColrLayers nodes
        let mut tree_nodes = Vec::new();
        let mut offset = 0u32;

        while offset < num_layers {
            let chunk_size = (num_layers - offset).min(MAX_PAINT_COLR_LAYER_COUNT as u32);
            tree_nodes.push(Paint::colr_layers(
                chunk_size as u8,
                first_layer_index + offset,
            ));
            offset += chunk_size;
        }

        // Now we have tree_nodes.len() PaintColrLayers nodes
        // If that's ≤255, we can create a single parent PaintColrLayers
        // Otherwise, we need to recursively build another level
        let num_tree_nodes = tree_nodes.len() as u32;
        let tree_first_index = self.layers.len() as u32;
        self.layers.extend(tree_nodes);

        // Recursively build the tree if needed
        self.build_n_ary_tree_internal(tree_first_index, num_tree_nodes)
    }

    /// Build the final LayerList.
    ///
    /// Returns None if no layers were added.
    pub fn build(self) -> Option<LayerList> {
        if self.layers.is_empty() {
            return None;
        }

        Some(LayerList::new(self.layers.len() as u32, self.layers))
    }
}
