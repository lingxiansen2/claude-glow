// Config window + system tray (native-windows-gui). Lets the user point at this
// machine's Claude settings.json, install/remove hooks, and toggle the overlay.

use native_windows_gui as nwg;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use crate::{config, hooks, overlay};

// Several fields are only held to keep the controls alive for the message loop.
#[allow(dead_code)]
struct Ui {
    window: nwg::Window,
    path_input: nwg::TextInput,
    browse_btn: nwg::Button,
    status_label: nwg::Label,
    install_btn: nwg::Button,
    uninstall_btn: nwg::Button,
    toggle_btn: nwg::Button,
    hint: nwg::Label,
    file_dialog: nwg::FileDialog,
    tray: nwg::TrayNotification,
    tray_menu: nwg::Menu,
    tray_show: nwg::MenuItem,
    tray_toggle: nwg::MenuItem,
    tray_exit: nwg::MenuItem,
    #[allow(dead_code)]
    icon: nwg::Icon,
}

fn refresh_status(ui: &Ui) {
    let path = ui.path_input.text();
    let installed = hooks::is_installed(&path);
    ui.status_label.set_text(if installed {
        "状态：已安装 ✓  （改动 settings.json 后需重启 Claude Code 会话生效）"
    } else {
        "状态：未安装 ✗  点击“安装 Hooks”写入当前路径的 settings.json"
    });
    let on = overlay::OVERLAY_ENABLED.load(Ordering::Relaxed);
    ui.toggle_btn
        .set_text(if on { "停止叠加层" } else { "启动叠加层" });
}

fn do_browse(ui: &Ui) {
    if ui.file_dialog.run(Some(&ui.window)) {
        if let Ok(item) = ui.file_dialog.get_selected_item() {
            ui.path_input.set_text(&item.to_string_lossy());
            refresh_status(ui);
        }
    }
}

fn do_install(ui: &Ui) {
    let path = ui.path_input.text();
    let mut cfg = config::load();
    cfg.settings_path = path.clone();
    let _ = config::save(&cfg);
    match hooks::install(&path, &config::exe_path()) {
        Ok(_) => {
            refresh_status(ui);
            nwg::modal_info_message(
                &ui.window,
                "Claude Glow",
                "已安装 hooks（保留了已有的其它 hooks）。\n请重启一次 Claude Code 会话后生效。",
            );
        }
        Err(e) => {
            nwg::modal_error_message(
                &ui.window,
                "Claude Glow",
                &format!("安装失败：{e}"),
            );
        }
    }
}

fn do_uninstall(ui: &Ui) {
    let path = ui.path_input.text();
    match hooks::uninstall(&path) {
        Ok(_) => {
            refresh_status(ui);
            nwg::modal_info_message(&ui.window, "Claude Glow", "已移除本工具的 hooks。");
        }
        Err(e) => {
            nwg::modal_error_message(&ui.window, "Claude Glow", &format!("移除失败：{e}"));
        }
    }
}

fn do_toggle(ui: &Ui) {
    let cur = overlay::OVERLAY_ENABLED.load(Ordering::Relaxed);
    overlay::OVERLAY_ENABLED.store(!cur, Ordering::Relaxed);
    refresh_status(ui);
}

pub fn run_app() {
    nwg::init().expect("Failed to init nwg");
    let _ = nwg::Font::set_global_family("Segoe UI");

    let cfg = config::load();

    let mut icon = nwg::Icon::default();
    let _ = nwg::Icon::builder()
        .source_bin(Some(include_bytes!("../assets/icon.ico")))
        .build(&mut icon);

    let mut window = nwg::Window::default();
    nwg::Window::builder()
        .size((560, 300))
        .position((420, 280))
        .title("Claude Status Glow")
        .icon(Some(&icon))
        .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
        .build(&mut window)
        .unwrap();

    let mut path_lbl = nwg::Label::default();
    nwg::Label::builder()
        .parent(&window)
        .text("Claude settings.json 路径：")
        .position((14, 14))
        .size((532, 22))
        .build(&mut path_lbl)
        .unwrap();

    let mut path_input = nwg::TextInput::default();
    nwg::TextInput::builder()
        .parent(&window)
        .text(&cfg.settings_path)
        .position((14, 40))
        .size((420, 28))
        .build(&mut path_input)
        .unwrap();

    let mut browse_btn = nwg::Button::default();
    nwg::Button::builder()
        .parent(&window)
        .text("浏览…")
        .position((442, 39)).size((104, 30))
        .build(&mut browse_btn)
        .unwrap();

    let mut status_label = nwg::Label::default();
    nwg::Label::builder()
        .parent(&window)
        .text("")
        .position((14, 82))
        .size((532, 24))
        .build(&mut status_label)
        .unwrap();

    let mut install_btn = nwg::Button::default();
    nwg::Button::builder()
        .parent(&window)
        .text("安装 Hooks")
        .position((14, 120)).size((168, 36))
        .build(&mut install_btn)
        .unwrap();

    let mut uninstall_btn = nwg::Button::default();
    nwg::Button::builder()
        .parent(&window)
        .text("卸载 Hooks")
        .position((196, 120)).size((168, 36))
        .build(&mut uninstall_btn)
        .unwrap();

    let mut toggle_btn = nwg::Button::default();
    nwg::Button::builder()
        .parent(&window)
        .text("停止叠加层")
        .position((378, 120)).size((168, 36))
        .build(&mut toggle_btn)
        .unwrap();

    let mut hint = nwg::Label::default();
    nwg::Label::builder()
        .parent(&window)
        .text("颜色：红=思考  橙=工具/联网  黄=等待确认  绿=完成。\n关闭窗口会最小化到托盘后台运行；托盘右键可重新打开或退出。")
        .position((14, 172))
        .size((532, 60))
        .build(&mut hint)
        .unwrap();

    let mut file_dialog = nwg::FileDialog::default();
    nwg::FileDialog::builder()
        .title("选择 Claude 的 settings.json")
        .action(nwg::FileDialogAction::Open)
        .filters("JSON(*.json)|All(*.*)")
        .build(&mut file_dialog)
        .unwrap();

    let mut tray = nwg::TrayNotification::default();
    nwg::TrayNotification::builder()
        .parent(&window)
        .icon(Some(&icon))
        .tip(Some("Claude Status Glow"))
        .build(&mut tray)
        .unwrap();

    let mut tray_menu = nwg::Menu::default();
    nwg::Menu::builder()
        .parent(&window)
        .popup(true)
        .build(&mut tray_menu)
        .unwrap();

    let mut tray_show = nwg::MenuItem::default();
    nwg::MenuItem::builder()
        .parent(&tray_menu)
        .text("显示配置")
        .build(&mut tray_show)
        .unwrap();

    let mut tray_toggle = nwg::MenuItem::default();
    nwg::MenuItem::builder()
        .parent(&tray_menu)
        .text("启动/停止叠加层")
        .build(&mut tray_toggle)
        .unwrap();

    let mut tray_exit = nwg::MenuItem::default();
    nwg::MenuItem::builder()
        .parent(&tray_menu)
        .text("退出")
        .build(&mut tray_exit)
        .unwrap();

    let ui = Rc::new(Ui {
        window,
        path_input,
        browse_btn,
        status_label,
        install_btn,
        uninstall_btn,
        toggle_btn,
        hint,
        file_dialog,
        tray,
        tray_menu,
        tray_show,
        tray_toggle,
        tray_exit,
        icon,
    });

    refresh_status(&ui);

    let ui_handler = ui.clone();
    let handler = nwg::full_bind_event_handler(&ui.window.handle, move |evt, evt_data, handle| {
        use nwg::Event as E;
        let ui = &ui_handler;
        match evt {
            E::OnButtonClick => {
                if handle == ui.browse_btn.handle {
                    do_browse(ui);
                } else if handle == ui.install_btn.handle {
                    do_install(ui);
                } else if handle == ui.uninstall_btn.handle {
                    do_uninstall(ui);
                } else if handle == ui.toggle_btn.handle {
                    do_toggle(ui);
                }
            }
            E::OnContextMenu => {
                if handle == ui.tray.handle {
                    let (x, y) = nwg::GlobalCursor::position();
                    ui.tray_menu.popup(x, y);
                }
            }
            E::OnMenuItemSelected => {
                if handle == ui.tray_show.handle {
                    ui.window.set_visible(true);
                    ui.window.restore();
                    refresh_status(ui);
                } else if handle == ui.tray_toggle.handle {
                    do_toggle(ui);
                } else if handle == ui.tray_exit.handle {
                    std::process::exit(0);
                }
            }
            E::OnWindowClose => {
                if handle == ui.window.handle {
                    // Hide to tray instead of quitting.
                    if let nwg::EventData::OnWindowClose(close_data) = &evt_data {
                        close_data.close(false);
                    }
                    ui.window.set_visible(false);
                }
            }
            _ => {}
        }
    });

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
}
