use super::tree_node_data::TreeNodeData;

pub struct TreeNode {
    pub data: TreeNodeData,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

impl TreeNode {
    pub const fn new(data: TreeNodeData) -> Self {
        Self {
            data,
            parent: None,
            children: vec![],
        }
    }
}
