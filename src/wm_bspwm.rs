use anyhow::{Context, Result};
use bspwmipc::reply::{Monitor, Node};
use bspwmipc::BspwmConnection;

use crate::DesktopWindow;

fn client_to_window(node: &Node, focused_node_id: u32) -> DesktopWindow {
    let rectangle = node.client.as_ref().unwrap().get_geometry();
    DesktopWindow {
        id: node.id.into(),
        x_window_id: Some(node.id as i32),
        pos: (rectangle.x.into(), rectangle.y.into()),
        size: (rectangle.width.into(), rectangle.height.into()),
        is_focused: node.id == focused_node_id,
    }
}

pub fn get_windows() -> Result<Vec<DesktopWindow>> {
    let mut connection = BspwmConnection::connect().context("Couldn't acquire bspwm connection")?;
    let monitors: Vec<Monitor> = connection.get_bspwm_state()?.monitors;
    let mut windows = vec![];
    for mon in monitors {
        for desk in mon
            .desktops
            .iter()
            .filter(|&d| d.id == mon.focused_desktop_id)
        {
            if let Some(tree) = &desk.root {
                let nodes: Vec<&Node> = tree
                    .traverse()
                    .into_iter()
                    .filter(|&n| !n.hidden && n.client.is_some())
                    .collect();
                for node in nodes {
                    windows.push(client_to_window(node, desk.focused_node_id));
                }
            };
        }
    }
    Ok(windows)
}

pub fn focus_window(window: &DesktopWindow) -> Result<()> {
    let mut connection = BspwmConnection::connect().context("Couldn't acquire bspwm connection")?;
    let command_str = format!("node {} -f", window.id);
    connection.raw_command(&command_str)?;
    Ok(())
}
