use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::iter::Iterator;
use std::time::Duration;
use xkbcommon::xkb;
use serde::Deserialize;
use std::io;

mod args;
mod utils;

//////////////////////////////////////////////////////////////////////////
//DesktopWindow -> HintDef
//id -> value
//is_focused -> highlight
//TODO: value should be a string.
#[derive(Deserialize, Debug)]
pub struct HintDef {
    value: i64,
    pos: (i32, i32),
    #[serde(default)]
    highlight: bool
}

//////////////////////////////////////////////////////////////////////////
//desktop_window -> hint_def
#[derive(Debug)]
pub struct RenderWindow<'a> {
    hint_def: &'a HintDef,
    cairo_context: cairo::Context,
    draw_pos: (f64, f64),
    rect: (i32, i32, i32, i32),
}

fn read_defs_from_stdin() -> Result<Vec<HintDef>> {
	let reader = serde_json::from_reader(io::stdin())?;
	Ok(reader)
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    let app_config = args::parse_args();

    /////////////////////////////////////////////////////////////////////////
    //new static stdin impl. here:
    //desktop_windows_raw -> hint_defs_raw
    // Get the windows from each specific window manager implementation.
    let hint_defs_raw = read_defs_from_stdin().context("Couldn't parse hints")?;

    /////////////////////////////////////////////////////////////////////////
    //desktop_windows -> hint_defs
    // Sort by position to make hint position more deterministic.
    let hint_defs = utils::sort_by_pos(hint_defs_raw);

    let (conn, screen_num) = xcb::Connection::connect(None).context("No Xorg connection")?;
    let setup = conn.get_setup();
    let screen = setup
        .roots()
        .nth(screen_num as usize)
        .context("Couldn't get screen")?;

    let values = [
        (xcb::CW_BACK_PIXEL, screen.black_pixel()),
        (
            xcb::CW_EVENT_MASK,
            xcb::EVENT_MASK_EXPOSURE
                | xcb::EVENT_MASK_KEY_PRESS
                | xcb::EVENT_MASK_BUTTON_PRESS
                | xcb::EVENT_MASK_BUTTON_RELEASE,
        ),
        (xcb::CW_OVERRIDE_REDIRECT, 1),
    ];

    // Assemble RenderWindows from DesktopWindows.
    let mut render_windows = HashMap::new();
    for hint_def in &hint_defs {
        // We need to estimate the font size before rendering because we want the window to only be
        // the size of the font.
        let hint = utils::get_next_hint(
            render_windows.keys().collect(),
            &app_config.hint_chars,
            hint_defs.len(),
        )
        .context("Couldn't get next hint")?;

        // Figure out how large the window actually needs to be.
        let text_extents = utils::extents_for_text(
            &hint,
            &app_config.font.font_family,
            app_config.font.font_size,
        )
        .context("Couldn't create extents for text")?;
        let (width, height, margin_width, margin_height) = {
            let margin_factor = 1.0 + 0.2;
            (
                (text_extents.width * margin_factor).round() as u16,
                (text_extents.height * margin_factor).round() as u16,
                ((text_extents.width * margin_factor) - text_extents.width) / 2.0,
                ((text_extents.height * margin_factor) - text_extents.height) / 2.0,
            )
        };

        // Due to the way cairo lays out text, we'll have to calculate the actual coordinates to
        // put the cursor. See:
        // https://www.cairographics.org/samples/text_align_center/
        // https://www.cairographics.org/samples/text_extents/
        // https://www.cairographics.org/tutorial/#L1understandingtext
        let draw_pos = (
            margin_width - text_extents.x_bearing,
            text_extents.height + margin_height - (text_extents.height + text_extents.y_bearing),
        );

        debug!(
            "Spawning RenderWindow for this HintDef: {:?}",
            hint_def
        );

        let mut x = hint_def.pos.0 as i16;
        let y = hint_def.pos.1 as i16;

        // If this is overlapping then we'll nudge the new RenderWindow a little bit out of the
        // way.
        let mut overlaps = utils::find_overlaps(
            render_windows.values().collect(),
            (x.into(), y.into(), width.into(), height.into()),
        );
        while !overlaps.is_empty() {
            x += overlaps.pop().unwrap().2 as i16;
            overlaps = utils::find_overlaps(
                render_windows.values().collect(),
                (x.into(), y.into(), width.into(), height.into()),
            );
        }

        let xcb_window_id = conn.generate_id();

        // Create the actual window.
        xcb::create_window(
            &conn,
            xcb::COPY_FROM_PARENT as u8,
            xcb_window_id,
            screen.root(),
            x,
            y,
            width,
            height,
            0,
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(),
            &values,
        );

        xcb::map_window(&conn, xcb_window_id);

        // Set transparency.
        let opacity_atom = xcb::intern_atom(&conn, false, "_NET_WM_WINDOW_OPACITY")
            .get_reply()
            .context("Couldn't create atom _NET_WM_WINDOW_OPACITY")?
            .atom();
        let opacity = (0xFFFFFFFFu64 as f64 * app_config.bg_color.3) as u64;
        xcb::change_property(
            &conn,
            xcb::PROP_MODE_REPLACE as u8,
            xcb_window_id,
            opacity_atom,
            xcb::ATOM_CARDINAL,
            32,
            &[opacity],
        );

        conn.flush();

        let mut visual =
            utils::find_visual(&conn, screen.root_visual()).context("Couldn't find visual")?;
        let cairo_xcb_conn = unsafe {
            cairo::XCBConnection::from_raw_none(
                conn.get_raw_conn() as *mut cairo_sys::xcb_connection_t
            )
        };
        let cairo_xcb_drawable = cairo::XCBDrawable(xcb_window_id);
        let raw_visualtype = &mut visual.base as *mut xcb::ffi::xcb_visualtype_t;
        let cairo_xcb_visual = unsafe {
            cairo::XCBVisualType::from_raw_none(raw_visualtype as *mut cairo_sys::xcb_visualtype_t)
        };
        let surface = cairo::XCBSurface::create(
            &cairo_xcb_conn,
            &cairo_xcb_drawable,
            &cairo_xcb_visual,
            width.into(),
            height.into(),
        )
        .context("Couldn't create Cairo Surface")?;
        let cairo_context =
            cairo::Context::new(&surface).context("Couldn't create Cairo Context")?;

        let render_window = RenderWindow {
            hint_def,
            cairo_context,
            draw_pos,
            rect: (x.into(), y.into(), width.into(), height.into()),
        };

        render_windows.insert(hint, render_window);
    }

    // Receive keyboard events.
    utils::snatch_keyboard(&conn, &screen, Duration::from_secs(1))?;

    // Receive mouse events.
    utils::snatch_mouse(&conn, &screen, Duration::from_secs(1))?;

    // Since we might have lots of windows on the desktop, it might be required
    // to enter a sequence in order to get to the correct window.
    // We'll have to track the keys pressed so far.
    let mut pressed_keys = String::default();
    let mut sequence = utils::Sequence::new(None);

    let mut closed = false;
    while !closed {
        let event = conn.wait_for_event();
        match event {
            None => {
                closed = true;
            }
            Some(event) => {
                let r = event.response_type();
                match r {
                    xcb::EXPOSE => {
                        for (hint, rw) in &render_windows {
                            utils::draw_hint_text(rw, &app_config, hint, &pressed_keys)
                                .context("Couldn't draw hint text")?;
                            conn.flush();
                        }
                    }
                    xcb::BUTTON_PRESS => {
                        closed = true;
                    }
                    xcb::KEY_RELEASE => {
                        let ksym = utils::get_pressed_symbol(&conn, &event);
                        let kstr = utils::convert_to_string(ksym)
                            .context("Couldn't convert ksym to string")?;
                        sequence.remove(kstr);
                    }
                    xcb::KEY_PRESS => {
                        let ksym = utils::get_pressed_symbol(&conn, &event);
                        let kstr = utils::convert_to_string(ksym)
                            .context("Couldn't convert ksym to string")?;

                        sequence.push(kstr.to_owned());

                        if app_config.hint_chars.contains(kstr) {
                            info!("Adding '{}' to key sequence", kstr);
                            pressed_keys.push_str(kstr);
                        } else {
                            warn!("Pressed key '{}' is not a valid hint characters", kstr);
                        }

                        info!("Current key sequence: '{}'", pressed_keys);

                        if ksym == xkb::KEY_Escape || app_config.exit_keys.contains(&sequence) {
                            info!("{:?} is exit sequence", sequence);
                            closed = true;
                            continue;
                        }

                        // Attempt to match the current sequence of keys as a string to the window
                        // hints shown.
                        // If there is an exact match, we're done. We'll then focus the window
                        // and exit. However, we also want to check whether there is still any
                        // chance to focus any windows from the current key sequence. If there
                        // is not then we will also just exit and focus no new window.
                        // If there still is a chance we might find a window then we'll just
                        // keep going for now.
                        if sequence.is_started() {
                            utils::remove_last_key(&mut pressed_keys, kstr);
                        } else if let Some(rw) = &render_windows.get(&pressed_keys) {
                            info!("Found matching window, focusing");
                            println!("{}", rw.hint_def.value);
                            closed = true;
                        } else if !pressed_keys.is_empty()
                            && render_windows.keys().any(|k| k.starts_with(&pressed_keys))
                        {
                            for (hint, rw) in &render_windows {
                                utils::draw_hint_text(rw, &app_config, hint, &pressed_keys)
                                    .context("Couldn't draw hint text")?;
                                conn.flush();
                            }
                            continue;
                        } else {
                            warn!("No more matches possible with current key sequence");
                            closed = app_config.exit_keys.is_empty();
                            utils::remove_last_key(&mut pressed_keys, kstr);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
