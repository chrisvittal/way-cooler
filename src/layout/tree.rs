//! Main module to handle the layout.
//! This is where the i3-specific code is.

use std::sync::{Mutex, MutexGuard, TryLockError};
use std::ptr;

use super::container::{Container, Handle, ContainerType};
use super::node::{Node};
use super::super::rustwlc::handle::{WlcView, WlcOutput};


pub type TreeResult = Result<(), TryLockError<MutexGuard<'static, Tree>>>;

const ERR_BAD_TREE: &'static str = "Layout tree was in an invalid configuration";

lazy_static! {
    static ref TREE: Mutex<Tree> = {
        Mutex::new(Tree{
            root: Node::new(Container::new_root()),
            active_container: ptr::null(),
        })
    };
}


pub struct Tree {
    root: Node,
    active_container: *const Node,
}

unsafe impl Send for Tree {}

impl Tree {
    /// Returns the currently viewed container.
    /// If multiple views are selected, the parent container they share is returned
    fn get_active_container(&self) -> Option<&Node> {
        if self.active_container.is_null() {
            None
        } else {
            unsafe {
                Some(&*self.active_container)
            }
        }
    }

    /// Get the monitor (output) that the active container is located on
    fn get_active_output(&self) -> Option<&Node> {
        if let Some(node) = self.get_active_container() {
            node.get_ancestor_of_type(ContainerType::Output)
        } else {
            None
        }
    }

    /// Get the workspace that the active container is located on
    fn get_active_workspace(&self) -> Option<&mut Node> {
        if let Some(container) = self.get_active_container() {
            //if let Some(child) = container.get_ancestor_of_type(ContainerType::Workspace) {
            //return child.get_children()[0].get_parent()

            //}
            // NOTE hack here, remove commented code above to make this work properly
            let parent = container.get_parent().expect(ERR_BAD_TREE);
            for child in parent.get_children_mut() {
                if child == container {
                    return Some(child);
                }
            }
        }
        return None
    }

    /// Find the workspace node that has the given name
    fn get_workspace_by_name(&self, name: &str) -> Option<&Node> {
        for child in self.root.get_children()[0].get_children() {
            if child.get_val().get_name().expect(ERR_BAD_TREE) != name {
                continue
            }
            return Some(child);
        }
        return None
    }

    /// Find the workspace node that has the given name, with a mutable reference
    fn get_workspace_by_name_mut(&mut self, name: &str) -> Option<&mut Node> {
        for child in self.root.get_children_mut()[0].get_children_mut() {
            if child.get_val().get_name().expect(ERR_BAD_TREE) != name {
                continue
            }
            return Some(child);
        }
        return None
    }

    /// Make a new output container with the given WlcOutput.
    /// This is done when a new monitor is added
    fn add_output(&mut self, wlc_output: WlcOutput) {
        self.root.new_child(Container::new_output(wlc_output));
    }

    /// Make a new workspace container with the given name.
    fn add_workspace(&mut self, name: String) {
        let workspace = Container::new_workspace(name.to_string());
        let mut index = 0;
        if let Some(output) = self.get_active_output() {
            for (cur_index, child) in self.root.get_children().iter().enumerate() {
                if child == output {
                    index = cur_index;
                    break;
                }
            }
        }
        self.root.get_children_mut()[index].new_child(workspace);
    }

    /// Make a new view container with the given WlcView, and adds it to
    /// the active workspace.
    fn add_view(&self, wlc_view: WlcView) {
        if let Some(current_workspace) = self.get_active_workspace() {
            trace!("Adding view {:?} to {:?}", wlc_view, current_workspace);
            current_workspace.new_child(Container::new_view(wlc_view));
        }
    }

    /// Remove the view container with the given view
    fn remove_view(&self, wlc_view: &WlcView) {
        if let Some(view) = self.root.find_view_by_handle(&wlc_view) {
            let parent = view.get_parent().expect(ERR_BAD_TREE);
            parent.remove_child(view);
        }
    }
}

pub fn add_output(wlc_output: WlcOutput) -> TreeResult {
    {
        let mut tree = try!(TREE.try_lock());
        tree.add_output(wlc_output);
    }
    try!(add_workspace("1".to_string()));
    try!(switch_workspace(&"1"));
    Ok(())
}

pub fn add_workspace(name: String) -> TreeResult {
    trace!("Adding new workspace to root");
    let mut tree = try!(TREE.lock());
    tree.add_workspace(name);
    Ok(())
}

pub fn add_view(wlc_view: WlcView) -> TreeResult {
    let tree = try!(TREE.lock());
    tree.add_view(wlc_view);
    Ok(())
}

pub fn remove_view(wlc_view: &WlcView) -> TreeResult {
    let tree = try!(TREE.lock());
    tree.remove_view(wlc_view);
    Ok(())
}

pub fn switch_workspace(name: &str) -> TreeResult {
    trace!("Switching to workspace {}", name);
    let mut tree = try!(TREE.lock());
    if let Some(old_workspace) = tree.get_active_workspace() {
        old_workspace.set_visibility(false);
    }
    let current_workspace: *const Node;
    {
        if let Some(_) = tree.get_workspace_by_name(name) {
            trace!("Found workspace {}", name);
        } else {
            trace!("Adding workspace {}", name);
            tree.add_workspace(name.to_string());
        }
        let new_current_workspace = tree.get_workspace_by_name_mut(name).expect(ERR_BAD_TREE);
        new_current_workspace.set_visibility(true);
        // Set the first view to be focused, so that the view is updated to this new workspace
        if new_current_workspace.get_children().len() > 0 {
            trace!("Focusing view");
            match new_current_workspace.get_children_mut()[0]
                .get_val().get_handle().expect(ERR_BAD_TREE) {
                Handle::View(view) => view.focus(),
                _ => {},
            }
        } else {
            WlcView::root().focus();
        }
        current_workspace = new_current_workspace as *const Node;
    }
    tree.active_container = current_workspace;
    Ok(())
}