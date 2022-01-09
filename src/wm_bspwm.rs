use anyhow::{Context, Result};
use bspwmipc::reply::{bspwmstate_t, node_t, client_t};
use bspwmipc::BspwmConnection;

use crate::DesktopWindow;

fn clienttowindow(id: u32, client: &client_t, focused: bool) -> DesktopWindow {
	let rectangle = client.getgeometry();
	let (pos_x, pos_y, size_x, size_y) = (rectangle.x, rectangle.y, rectangle.width, rectangle.height);
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
	let state: bspwmstate_t = connection.get_bspwm_state();
	let mut windows = vec![];
	for mon in state.monitors {
		for desk in mon.desktops {
			if desk.id == mon.focusedDesktopId {
				if let Some(tree) = desk.root {
					let nodes: Vec<&node_t> = tree.traverse();
					for node in nodes {
						if node.client.is_some() && !node.hidden {
							let focused: bool = node.id == desk.focusedNodeId;
							let window: DesktopWindow = clienttowindow(node.id, node.client.as_ref().unwrap(), focused);
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
	connection.raw_command(&command_str);
	Ok(())
}
