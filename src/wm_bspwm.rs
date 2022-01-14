use anyhow::{Context, Result};
use bspwmipc::reply::{BspwmState, Client, Node};
use bspwmipc::BspwmConnection;

use crate::DesktopWindow;

fn client_to_window(id: u32, client: &Client, focused: bool) -> DesktopWindow {
    let rectangle = client.get_geometry();
    let (pos_x, pos_y, size_x, size_y) =
        (rectangle.x, rectangle.y, rectangle.width, rectangle.height);
    let xwinid: Option<i32> = Some(id as i32);
    DesktopWindow {
        id: id.into(),
        x_window_id: xwinid,
        pos: (pos_x.into(), pos_y.into()),
        size: (size_x.into(), size_y.into()),
        is_focused: focused,
    }
}

pub fn get_windows() -> Result<Vec<DesktopWindow>> {
    let mut connection = BspwmConnection::connect().context("Couldn't acquire bspwm connection")?;
    let state: BspwmState = connection.get_bspwm_state()?;
    let mut windows = vec![];
    for mon in state.monitors {
        for desk in mon.desktops {
            if desk.id == mon.focused_desktop_id {
                if let Some(tree) = desk.root {
                    let nodes: Vec<&Node> = tree.traverse();
                    for node in nodes {
                        if node.client.is_some() && !node.hidden {
                            let focused: bool = node.id == desk.focused_node_id;
                            let window: DesktopWindow =
                                client_to_window(node.id, node.client.as_ref().unwrap(), focused);
                            windows.push(window);
                        }
                    }
                }
            }
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
