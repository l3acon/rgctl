//! Read-only node payload lookup shared by live backend and cold mmap store.

use rgctl_error::Result;
use rgctl_graph::backend::{GraphBackend, MemoryBackend};
use rgctl_graph::schema::{Node, NodeType};
use uuid::Uuid;

use crate::cold_metadata::ColdMetadataDb;

/// Minimal node-payload access for analysis stages that no longer need edge storage.
pub trait NodeLookup {
    /// Lookup a node by UUID.
    fn get_node(&self, id: Uuid) -> Result<Option<Node>>;

    /// Hot `(name, file_path)` when the backend can avoid cold_ext decode.
    fn node_name_and_path_opt(&self, id: Uuid) -> Result<Option<(String, Option<String>)>> {
        Ok(self.get_node(id)?.map(|n| {
            (
                n.name.to_string(),
                n.file_path.as_ref().map(|s| s.to_string()),
            )
        }))
    }

    /// Total node count.
    fn node_count(&self) -> usize;

    /// Total edge count (may come from snapshot header after edges were released).
    fn edge_count(&self) -> usize;

    /// Visit every node (may deserialize cold extension blobs).
    fn for_each_node(&self, f: &mut dyn FnMut(&Node)) -> Result<()>;

    /// Collect nodes of a given type.
    fn collect_nodes_by_type(&self, node_type: NodeType) -> Result<Vec<Node>> {
        let mut out = Vec::new();
        self.for_each_node(&mut |node| {
            if node.node_type == node_type {
                out.push(node.clone());
            }
        })?;
        Ok(out)
    }
}

impl NodeLookup for MemoryBackend {
    fn get_node(&self, id: Uuid) -> Result<Option<Node>> {
        GraphBackend::get_node(self, id)
    }

    fn node_count(&self) -> usize {
        MemoryBackend::node_count(self)
    }

    fn edge_count(&self) -> usize {
        MemoryBackend::edge_count(self)
    }

    fn for_each_node(&self, f: &mut dyn FnMut(&Node)) -> Result<()> {
        MemoryBackend::for_each_node(self, |n| f(n))
    }

    fn collect_nodes_by_type(&self, node_type: NodeType) -> Result<Vec<Node>> {
        MemoryBackend::collect_nodes_by_type(self, node_type)
    }
}

impl NodeLookup for ColdMetadataDb {
    fn get_node(&self, id: Uuid) -> Result<Option<Node>> {
        ColdMetadataDb::get_node(self, id)
    }

    fn node_name_and_path_opt(&self, id: Uuid) -> Result<Option<(String, Option<String>)>> {
        ColdMetadataDb::node_name_and_path(self, id)
    }

    fn node_count(&self) -> usize {
        ColdMetadataDb::node_count(self)
    }

    fn edge_count(&self) -> usize {
        ColdMetadataDb::edge_count(self)
    }

    fn for_each_node(&self, f: &mut dyn FnMut(&Node)) -> Result<()> {
        ColdMetadataDb::for_each_node_seq(self, f)
    }

    fn collect_nodes_by_type(&self, node_type: NodeType) -> Result<Vec<Node>> {
        ColdMetadataDb::collect_nodes_by_type(self, node_type)
    }
}
