use crate::types::{FileEntry, FileStatus, SelectableItem, TreeNode};
use std::path::{Path, PathBuf};

/// Build a tree from a list of file entries
pub fn build_tree(files: &[FileEntry], hide_dotfiles: bool) -> TreeNode {
    let mut root = TreeNode::root();

    for file in files {
        // Skip dotfiles if requested
        if hide_dotfiles && is_dotfile(&file.path) {
            continue;
        }

        add_to_tree(&mut root, &file.path, &file.status);
    }

    root.sort_children();
    root
}

/// Check if a path is a dotfile/dotdir
fn is_dotfile(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    })
}

/// Add a file path to the tree, creating intermediate directories
fn add_to_tree(node: &mut TreeNode, path: &Path, status: &FileStatus) {
    let components: Vec<_> = path.components().collect();
    add_to_tree_recursive(node, &components, 0, status, path.to_path_buf());
}

fn add_to_tree_recursive(
    node: &mut TreeNode,
    components: &[std::path::Component],
    depth: usize,
    status: &FileStatus,
    full_path: PathBuf,
) {
    if depth >= components.len() {
        return;
    }

    let name = components[depth]
        .as_os_str()
        .to_str()
        .unwrap_or("")
        .to_string();

    let is_last_component = depth == components.len() - 1;

    // Build path up to this component
    let component_path: PathBuf = components[..=depth]
        .iter()
        .map(|c| c.as_os_str())
        .collect();

    // Find or create child node
    let child_idx = node.children.iter().position(|c| c.name == name);

    match child_idx {
        Some(idx) => {
            // Node exists, merge status if it's the final component
            if is_last_component {
                node.children[idx].status.merge(status);
            } else {
                // Continue recursing
                add_to_tree_recursive(
                    &mut node.children[idx],
                    components,
                    depth + 1,
                    status,
                    full_path,
                );
            }
        }
        None => {
            // Create new node
            let new_node = if is_last_component {
                TreeNode::new_file(name, full_path, status.clone())
            } else {
                let mut dir = TreeNode::new_directory(name, component_path.clone());
                add_to_tree_recursive(&mut dir, components, depth + 1, status, full_path);
                dir
            };
            node.children.push(new_node);
        }
    }
}

/// Flatten tree into a list of selectable items (respecting expanded state)
pub fn flatten_tree(root: &TreeNode, hide_dotfiles: bool) -> Vec<SelectableItem> {
    let mut items = Vec::new();
    flatten_node(root, 0, true, Vec::new(), &mut items, hide_dotfiles);
    items
}

fn flatten_node(
    node: &TreeNode,
    depth: usize,
    is_last: bool,
    ancestors: Vec<bool>,
    items: &mut Vec<SelectableItem>,
    hide_dotfiles: bool,
) {
    // Skip root node itself, but process its children
    if node.name != "." {
        // Skip dotfiles if requested
        if hide_dotfiles && node.name.starts_with('.') {
            return;
        }

        items.push(SelectableItem::from_node(node, depth, is_last, ancestors.clone()));
    }

    // Only process children if expanded (or if this is root)
    if node.is_expanded || node.name == "." {
        let child_count = node.children.len();
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i == child_count - 1;
            let mut child_ancestors = ancestors.clone();
            if node.name != "." {
                child_ancestors.push(is_last);
            }
            flatten_node(
                child,
                if node.name == "." { depth } else { depth + 1 },
                child_is_last,
                child_ancestors,
                items,
                hide_dotfiles,
            );
        }
    }
}

/// Find a node in the tree by path
pub fn find_node_mut<'a>(root: &'a mut TreeNode, path: &Path) -> Option<&'a mut TreeNode> {
    if root.full_path == path {
        return Some(root);
    }

    for child in &mut root.children {
        if let Some(found) = find_node_mut(child, path) {
            return Some(found);
        }
    }

    None
}

/// Toggle expanded state for a node at the given path
pub fn toggle_expanded(root: &mut TreeNode, path: &Path) -> bool {
    if let Some(node) = find_node_mut(root, path) {
        if !node.is_file {
            node.toggle_expanded();
            return true;
        }
    }
    false
}
