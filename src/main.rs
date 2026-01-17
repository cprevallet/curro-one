/*
 * This program plots Activity files stored in the Garmin (Dynastream)
 * FIT format.
 *
 * License:
 *
 * Permission is granted to copy, use, and distribute for any commercial
 * or noncommercial purpose in accordance with the requirements of
 * version 2.0 of the GNU General Public license.
 *
 * This package is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this package; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA  02110-1301 USA
 *
 * On Debian systems, the complete text of the GNU General
 * Public License can be found in `/usr/share/common-licenses/GPL-2'.
 *
 * - Craig S. Prevallet, December, 2025
 */
#![windows_subsystem = "windows"]
mod config;
mod data;
mod gui;
mod i18n;

use crate::config::{
    APP_ID, AUTHOR, COPYRIGHT, ICON_NAME, PROGRAM_NAME, TESTER1, TESTER2, TESTER3, WindowConfig,
    save_config,
};
use crate::gui::{
    UserInterface, connect_interactive_widgets, construct_views_from_data, instantiate_graph_cache,
    instantiate_map_cache, instantiate_ui,
};
use crate::i18n::tr;
use gtk4::glib::clone;
use gtk4::prelude::*;
use gtk4::{
    ButtonsType, FileChooserAction, FileChooserNative, License, MessageDialog, MessageType,
    ResponseType, gio,
};
use libadwaita::{Application, WindowTitle};
use semver::{BuildMetadata, Prerelease};
use std::error::Error;
use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;
use std::rc::Rc;

// Only God and I knew what this was doing when I wrote it.
// Now only God knows.
// Program entry point.
fn main() {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_activate(build_gui_no_files);
    app.connect_open(|app, files, _| {
        build_gui(app, files, "");
    });
    app.run();
}

// Create and present a modal MessageDialog when supplied a text string.
pub fn show_error_dialog<W: IsA<gtk4::Window>>(parent: &W, text_str: String) {
    // Create the MessageDialog
    let dialog = MessageDialog::builder()
        // Set the parent window to make it modal relative to the main window
        .transient_for(parent)
        // Set it to be modal (blocks interaction with the parent window)
        .modal(true)
        // Specify the type of dialog (e.g., Error, Info, Warning)
        .message_type(MessageType::Error)
        // Specify the button layout (e.g., Ok, YesNo, OkCancel)
        .buttons(ButtonsType::Ok)
        // Set the main text message
        // .text("Error: Failed to process file.")
        .text(text_str)
        // Set the secondary, explanatory text (optional)
        // .secondary_text(Some(
        //     "The selected FIT file could not be parsed due to an unexpected format or corruption.",
        // ))
        .build();
    // Connect to the response signal to handle button clicks (e.g., when "OK" is pressed)
    dialog.connect_response(|dialog, _response| {
        // ResponseType::Ok is returned when the "OK" button (from ButtonsType::Ok) is clicked.
        // Destroy the dialog when a response is received
        dialog.close();
    });
    // Display the dialog.
    dialog.present();
}

// Update window title.
fn update_window_title(ui: &UserInterface, path_str: &str) {
    let c_title = ui.win.title().unwrap().to_string().to_owned();
    let mut pfx = c_title
        .chars()
        .take_while(|&ch| ch != ':')
        .collect::<String>();
    pfx.push_str(":");
    pfx.push_str(" ");
    pfx.push_str(&path_str);
    let window_title = WindowTitle::new(&pfx.to_string(), "");
    ui.header_bar.set_title_widget(Some(&window_title));
}

// Get the file handle from the command line.
fn get_file_handle_from_command_line(
    file: &gtk4::gio::File,
    ui: &Rc<UserInterface>,
) -> Option<File> {
    if let Some(file_path) = file.path() {
        let path_buf = file.path().unwrap();
        let path_str = path_buf.to_string_lossy();
        let file_result = File::open(&file_path);
        match file_result {
            Ok(mut file) => {
                update_window_title(&ui, &path_str);
                tie_it_all_together(&mut file, &ui);
                return Some(file);
            }
            Err(error) => match error.kind() {
                // Handle specifically "Not Found"
                ErrorKind::NotFound => {
                    println!("File not found.");
                    return None;
                }
                _ => {
                    println!("Error unknown. Not a Fit File? Permissions?");
                    return None;
                }
            },
        };
    } else {
        return None;
    }
}

// Get the file handle from a dialog.
fn get_file_handle_from_dialog(dialog: &FileChooserNative, ui: &UserInterface) -> Option<File> {
    // Extract the file path
    if let Some(file) = dialog.file() {
        if let Some(path) = file.path() {
            let path_str = path.to_string_lossy();
            // Get values from fit file.
            let file_result = File::open(&*path_str);
            match file_result {
                Ok(file) => {
                    update_window_title(&ui, &path_str);
                    return Some(file);
                }
                Err(error) => match error.kind() {
                    // Handle specifically "Not Found"
                    ErrorKind::NotFound => {
                        show_error_dialog(&ui.win, tr("MESSAGE_FILE_NOT_FOUND", None));
                        return None;
                    }
                    _ => {
                        show_error_dialog(&ui.win, tr("MESSAGE_PERMISSIONS", None));
                        return None;
                    }
                },
            };
        } else {
            return None;
        }
    } else {
        return None;
    }
}

// Get the data, create the caches, construct the views, and connect the interactive widgets.
fn tie_it_all_together(file: &mut File, ui: &Rc<UserInterface>) {
    if let Ok(data) = fitparser::from_reader(file) {
        // Create a map cache.
        let map_cache = instantiate_map_cache(&data);
        // Wrap the MapCache in an Rc for shared ownership.
        let mc_rc = Rc::new(map_cache);
        // Create a graph cache.
        let graph_cache = instantiate_graph_cache(&data, &ui);
        // Wrap the GraphCache in an Rc for shared ownership.
        let gc_rc = Rc::new(graph_cache);
        construct_views_from_data(&ui, &data, &mc_rc, &gc_rc);
        connect_interactive_widgets(&ui, &data, &mc_rc, &gc_rc);
        ui.curr_pos_scale.grab_focus();
    }
}

// Wrapper for build_gui to handle no files from command line.
fn build_gui_no_files(app: &Application) {
    build_gui(&app, &[], "");
}
// Instantiate the user-interface views and handle callbacks.
fn build_gui(app: &Application, files: &[gtk4::gio::File], _: &str) {
    // Instantiate the views.
    let ui_original = instantiate_ui(app);
    // Create a new reference count for the user_interface structure.
    // This gets a little tricky.  We need to create a new reference
    // counted pointer, ui_rc, from the original object and clone it
    // twice so that we may *SHARE* the contents of ui_original in two
    // different closures ("button-clicked" and "native window response").
    let ui_rc = Rc::new(ui_original);
    let ui1 = Rc::clone(&ui_rc);
    ui_rc.win.present();

    // If the user has provided a file name on the command line - use the first file.
    if files.len() > 0 {
        get_file_handle_from_command_line(&files[0], &ui_rc);
    }

    // Handle callbacks for btn and about_btn.
    let open_action = gio::SimpleAction::new("open", None);
    open_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            // 1. Create the Native Dialog
            // Notice the arguments: Title, Parent Window, Action, Accept Label, Cancel Label
            let native = FileChooserNative::new(
                Some(&tr("OPEN_FILE_BUTTON_LABEL", None)),
                Some(&ui1.win),
                FileChooserAction::Open,
                Some("Open"),   // Custom label for the "OK" button
                Some("Cancel"), // Custom label for the "Cancel" button
            );

            let ui2 = Rc::clone(&ui_rc);
            // 2. Connect to the response signal
            native.connect_response(clone!(
                #[strong]
                ui2,
                move |dialog, response| {
                    if response == ResponseType::Accept {
                        let fh = get_file_handle_from_dialog(&dialog, &ui2);
                        if fh.is_some() {
                            let mut file = fh.unwrap();
                            tie_it_all_together(&mut file, &ui2);
                            // unlike FileChooserDialog, 'native' creates a transient reference.
                            // It's good practice to drop references, but GTK handles the cleanup
                            // once it goes out of scope or the window closes.
                        }
                    }
                },
            ));
            // 3. Show the dialog
            native.show();
        },
    )); //open action

    // Connect the action to the widget and the shortcut key.
    app.add_action(&open_action);
    ui1.btn.set_action_name(Some("app.open"));
    app.set_accels_for_action("app.open", &["<Primary>o"]);

    let about_action = gio::SimpleAction::new("about", None);
    about_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            // The compile-time::datetime_str!() macro provides a &str literal at compile time,
            // e.g., "2025-12-10T18:36:25Z".
            let datetime_raw = compile_time::datetime_str!();
            //  Format it to be semver compliant. Build metadata identifiers can only contain
            //  ASCII alphanumerics and hyphens (`-`). The SemVer specification states:
            //  "Build metadata MAY be denoted by appending a plus sign and a series of
            //  dot separated identifiers immediately following the patch or pre-release version.
            //  Identifiers MUST comprise only ASCII alphanumerics and hyphens [0-9A-Za-z-]."
            // A common approach is to strip the non-compliant characters ('T', ':', 'Z')
            // and use the resulting string as a single build metadata identifier.
            let build_metadata_str: String = datetime_raw
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-') // Keep A-Z, a-z, 0-9, and '-'
                .collect();
            // The resulting string will be something like "2025-12-10183625".
            // This is a single, valid build metadata identifier.
            // Set the dynamic build metadata
            let build = BuildMetadata::new(&build_metadata_str).unwrap();
            // Get the version string injected by the build.rs script at compile time
            const VERSION_STRING: &str = env!("CRATE_VERSION");
            let mut semantic_version =
                semver::Version::parse(VERSION_STRING).unwrap_or_else(|_| {
                    // Fallback to a default if parsing fails (shouldn't happen with valid Cargo.toml)
                    semver::Version::new(0, 0, 0)
                });
            // Set the semantic_version variable for the dialog.
            semantic_version.build = build;
            semantic_version.pre = Prerelease::new("beta.1").unwrap();
            let comments: String = tr("ABOUT_DIALOG_COMMENT", None);
            let copyright: String = tr("COPYRIGHT", None);
            let rights: String = copyright.to_owned() + &COPYRIGHT;
            let dialog = gtk4::AboutDialog::builder()
                .transient_for(&ui1.win)
                .modal(true)
                .program_name(PROGRAM_NAME)
                .logo_icon_name(ICON_NAME)
                .license_type(License::Gpl20)
                .wrap_license(true)
                .version(semantic_version.to_string())
                .copyright(rights)
                .comments(comments)
                .authors(vec![
                    AUTHOR.to_string(),
                    TESTER1.to_string(),
                    TESTER2.to_string(),
                    TESTER3.to_string(),
                ])
                .build();
            dialog.present();
        }
    )); // about-action
    app.add_action(&about_action);
    app.set_accels_for_action("app.about", &["<Primary>a"]);
    ui1.about_btn.set_action_name(Some("app.about"));

    ui1.win.connect_close_request(clone!(
        #[strong]
        ui1,
        move |window| {
            let config_path = Path::new(&ui1.settings_file);
            let current_config = WindowConfig {
                width: window.width(),
                height: window.height(),
                main_split: ui1.main_pane.position(),
                right_frame_split: ui1.right_frame_pane.position(),
                left_frame_split: ui1.left_frame_pane.position(),
                units_index: ui1.units_widget.selected(),
                tile_source_widget_index: ui1.tile_source_widget.selected(),
            };
            match save_config(&current_config, config_path) {
                Ok(_) => glib::signal::Propagation::Proceed,
                Err(e) => {
                    show_error_dialog(window, e.to_string());
                    glib::signal::Propagation::Proceed
                }
            }
        }
    )); //window-close

    let forward_action = gio::SimpleAction::new("forward", None);
    forward_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let adj = ui1.curr_pos_scale.adjustment();
            let new_val = adj.value() + adj.step_increment();
            if new_val <= adj.upper() {
                adj.set_value(new_val);
                // Ensure focus stays or returns to the scale so
                // the user can continue using arrow keys too.
                ui1.curr_pos_scale.grab_focus();
            }
        }
    )); // forward-action
    app.add_action(&forward_action);
    app.set_accels_for_action("app.forward", &["<Primary>f"]);

    let backward_action = gio::SimpleAction::new("backward", None);
    backward_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let adj = ui1.curr_pos_scale.adjustment();
            let new_val = adj.value() - adj.step_increment();
            if new_val <= adj.upper() {
                adj.set_value(new_val);
                // Ensure focus stays or returns to the scale so
                // the user can continue using arrow keys too.
                ui1.curr_pos_scale.grab_focus();
            }
        }
    )); // backward-action
    app.add_action(&backward_action);
    app.set_accels_for_action("app.backward", &["<Primary>b"]);

    // Action for Page Down
    let page_down_action = gio::SimpleAction::new("page_down", None);
    page_down_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let adj = ui1.scrolled_window.vadjustment();
            let new_val = (adj.value() + adj.page_increment()).min(adj.upper() - adj.page_size());
            adj.set_value(new_val);
        }
    ));
    app.add_action(&page_down_action);
    app.set_accels_for_action("app.page_up", &["Page_Up"]);

    // Action for Page Up
    let page_up_action = gio::SimpleAction::new("page_up", None);
    page_up_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let adj = ui1.scrolled_window.vadjustment();
            let new_val = (adj.value() - adj.page_increment()).max(adj.lower());
            adj.set_value(new_val);
        }
    ));
    app.add_action(&page_up_action);
    app.set_accels_for_action("app.page_down", &["Page_Down"]);

    let map_zoom_in_action = gio::SimpleAction::new("map_zoom_in", None);
    map_zoom_in_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let viewport = ui1.map.viewport().unwrap();
            let current_zoom = viewport.zoom_level();
            if current_zoom < 20.0 {
                viewport.set_zoom_level(current_zoom + 1.0);
            }
        }
    )); // map zoom in action
    app.add_action(&map_zoom_in_action);
    app.set_accels_for_action("app.map_zoom_in", &["<Primary>F5"]);

    let map_zoom_out_action = gio::SimpleAction::new("map_zoom_out", None);
    map_zoom_out_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let viewport = ui1.map.viewport().unwrap();
            let current_zoom = viewport.zoom_level();
            if current_zoom > 1.0 {
                viewport.set_zoom_level(current_zoom - 1.0);
            }
        }
    )); //map zoom out action
    app.add_action(&map_zoom_out_action);
    app.set_accels_for_action("app.map_zoom_out", &["<Primary>F6"]);

    let y_zoom_in_action = gio::SimpleAction::new("y_zoom_in", None);
    y_zoom_in_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let adj = &ui1.y_zoom_adj;
            let new_val = adj.value() + adj.step_increment();
            if new_val <= adj.upper() {
                adj.set_value(new_val);
            }
        }
    )); // y_zoom_in-action
    app.add_action(&y_zoom_in_action);
    app.set_accels_for_action("app.y_zoom_in", &["<Primary>F7"]);
    let y_zoom_out_action = gio::SimpleAction::new("y_zoom_out", None);
    y_zoom_out_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let adj = &ui1.y_zoom_adj;
            let new_val = adj.value() - adj.step_increment();
            if new_val <= adj.upper() {
                adj.set_value(new_val);
            }
        }
    )); // y_zoom_out-action
    app.add_action(&y_zoom_out_action);
    app.set_accels_for_action("app.y_zoom_out", &["<Primary>F8"]);

    let unit_toggle_action = gio::SimpleAction::new("toggle-units", None);
    unit_toggle_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            // Get current index: 0 is usually US, 1 is Metric (based on your Units enum)
            let current = ui1.units_widget.selected();
            if current == 0 {
                ui1.units_widget.set_selected(1);
            } else {
                ui1.units_widget.set_selected(0);
            }
        }
    )); //units toggle action
    app.add_action(&unit_toggle_action);
    app.set_accels_for_action("app.toggle-units", &["<Primary>u"]);

    // Action to cycle tile source FORWARD (Ctrl+n)
    let tile_next_action = gio::SimpleAction::new("tile_next", None);
    tile_next_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let n_items = ui1.tile_source_widget.model().unwrap().n_items();
            if n_items > 0 {
                let current = ui1.tile_source_widget.selected();
                let next = (current + 1) % n_items; // Wrap to 0 after last item
                ui1.tile_source_widget.set_selected(next);
            }
        }
    ));
    app.add_action(&tile_next_action);
    app.set_accels_for_action("app.tile_next", &["<Primary>n"]);

    // Action to cycle tile source BACKWARD (Ctrl+p)
    let tile_prev_action = gio::SimpleAction::new("tile_prev", None);
    tile_prev_action.connect_activate(clone!(
        #[strong]
        ui1,
        move |_, _| {
            let n_items = ui1.tile_source_widget.model().unwrap().n_items();
            if n_items > 0 {
                let current = ui1.tile_source_widget.selected();
                let prev = if current == 0 {
                    n_items - 1
                } else {
                    current - 1
                };
                ui1.tile_source_widget.set_selected(prev);
            }
        }
    ));
    app.add_action(&tile_prev_action);
    app.set_accels_for_action("app.tile_prev", &["<Primary>p"]);
} // build_gui
