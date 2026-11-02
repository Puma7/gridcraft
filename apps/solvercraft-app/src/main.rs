//! Standalone local Solvercraft application; mathematical work stays in the shared SDK.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use gridcraft_ui_egui::{Services, SheetApp, doc_windows::DocWindows};
use std::{cell::RefCell, rc::Rc};

struct App {
    sheet: SheetApp,
    windows: DocWindows,
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sheet.logic(ctx);
        self.windows.logic(&mut self.sheet, ctx);
        let title = self.sheet.session.active().map(|doc| doc.display_title()).unwrap_or_else(|| "Workbook".into());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{title} — Solvercraft")));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        solvercraft_app::heading(&mut self.sheet, ui);
        self.windows.show(&mut self.sheet, ui);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.sheet.raw_input_hook(ctx, input);
        for file in std::mem::take(&mut input.dropped_files) {
            if let Some(path) = file.path().to_str().filter(|path| !path.is_empty()) {
                self.sheet.open_path(path);
            }
        }
    }
}

fn services() -> Services {
    let clipboard = Rc::new(RefCell::new(None::<arboard::Clipboard>));
    let read = Rc::clone(&clipboard);
    Services {
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Workbook", &["xlsx", "xlsm", "csv", "tsv", "json"])
                .pick_file()
                .and_then(|path| path.to_str().map(str::to_owned))
        })),
        pick_save: Some(Box::new(|suggested| {
            rfd::FileDialog::new()
                .set_file_name(suggested)
                .add_filter("Workbook (.xlsx)", &["xlsx"])
                .add_filter("CSV (.csv)", &["csv"])
                .add_filter("PDF (.pdf)", &["pdf"])
                .save_file()
                .and_then(|path| path.to_str().map(str::to_owned))
        })),
        pick_picture: Some(Box::new(|| {
            rfd::FileDialog::new().add_filter("Picture", &["png", "jpg", "jpeg"]).pick_file().and_then(|path| path.to_str().map(str::to_owned))
        })),
        copy_html: Some(Box::new(move |html, text| {
            let mut clipboard = clipboard.try_borrow_mut().map_err(|error| error.to_string())?;
            if clipboard.is_none() {
                *clipboard = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
            }
            clipboard.as_mut().ok_or_else(|| "Clipboard unavailable".to_string())?.set_html(html, Some(text)).map_err(|error| error.to_string())
        })),
        paste_text: Some(Box::new(move || {
            let mut clipboard = read.try_borrow_mut().ok()?;
            if clipboard.is_none() {
                *clipboard = arboard::Clipboard::new().ok();
            }
            clipboard.as_mut()?.get_text().ok()
        })),
        open_url: Some(Box::new(open_with_system)),
        ..Services::default()
    }
}

fn open_with_system(target: &str) {
    let gridcraft_engine::links::LinkKind::External(target) = gridcraft_engine::links::classify(target) else {
        return;
    };
    let command = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(error) = std::process::Command::new(command).arg(target.as_str()).spawn() {
        let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("Solvercraft could not open the link: {error}\n"));
    }
}

#[derive(Default)]
struct Arguments {
    blank: bool,
    file: Option<String>,
}

fn arguments(args: &[String]) -> Result<Option<Arguments>, String> {
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("Solvercraft {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Solvercraft — local optimization and simulation\n\nUsage: solvercraft [--blank] [workbook.xlsx]\n\nWithout a file, opens original optimization and simulation examples.\nNo network service is started. External agents use solvercraft-mcp."
        );
        return Ok(None);
    }
    let mut parsed = Arguments::default();
    for arg in args {
        if arg == "--blank" {
            parsed.blank = true;
        } else if arg.starts_with('-') {
            return Err(format!("Unknown option: {arg}. Use --help."));
        } else if parsed.file.replace(arg.clone()).is_some() {
            return Err("Open one workbook at startup; use File > Open for more.".into());
        }
    }
    Ok(Some(parsed))
}

fn main() -> eframe::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let arguments = match arguments(&args) {
        Ok(Some(arguments)) => arguments,
        Ok(None) => return Ok(()),
        Err(error) => return Err(eframe::Error::AppCreation(error.into())),
    };
    let (mut sheet, selected) = solvercraft_app::workbench(arguments.blank || arguments.file.is_some(), services())
        .map_err(|error| eframe::Error::AppCreation(error.into()))?;
    sheet.ui.language = gridcraft_ui_egui::i18n::Language::system();
    sheet.watch_desktop_theme();
    if let Some(path) = arguments.file {
        sheet.open_path(&path);
    }
    let viewport = egui::ViewportBuilder::default()
        .with_title("Solvercraft")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([820.0, 540.0])
        .with_app_id("io.github.puma7.solvercraft")
        .with_drag_and_drop(true)
        .with_icon(solvercraft_app::icon());
    let options = eframe::NativeOptions { viewport: viewport.clone(), ..Default::default() };
    // Never enumerate Vulkan by default on Windows: some drivers crash before
    // adapter selection. FXC is supplied by Windows and needs no companion DLL.
    #[cfg(target_os = "windows")]
    let options = {
        let mut options = options;
        if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
            create.instance_descriptor.backends = eframe::wgpu::Backends::from_env().unwrap_or(eframe::wgpu::Backends::DX12);
            create.instance_descriptor.backend_options.dx12.shader_compiler =
                eframe::wgpu::Dx12Compiler::from_env().unwrap_or(eframe::wgpu::Dx12Compiler::Fxc);
        }
        options
    };
    eframe::run_native(
        "Solvercraft",
        options,
        Box::new(move |creation| {
            SheetApp::setup_context(&creation.egui_ctx, sheet.ui.dark);
            if let Err(error) = solvercraft_app::select_model(&mut sheet, &creation.egui_ctx, selected.as_deref()) {
                sheet.message = Some(("Solvercraft".into(), error));
            }
            let windows = DocWindows::new(&mut sheet, viewport, true);
            Ok(Box::new(App { sheet, windows }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_options_and_ambiguous_startup_files_fail() {
        assert!(arguments(&["--unknown".into()]).is_err());
        assert!(arguments(&["a.xlsx".into(), "b.xlsx".into()]).is_err());
    }
}
