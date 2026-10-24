use egui::{Align, CornerRadius, Frame, Layout, Margin, RichText, Stroke};

use crate::app::App;
use crate::i18n::gettext;
use crate::model::Action;
use crate::theme::{self, Icon};
use crate::updates::DownloadState;

pub fn show(app: &mut App, ctx: &egui::Context) {
    if !app.show_update {
        return;
    }
    let Some(release) = app.update.clone() else {
        app.show_update = false;
        return;
    };
    let palette = app.palette;
    let locale = app.locale;
    let title = gettext(locale, "Update Spotifast");
    let mut close = ctx.input(|input| input.key_pressed(egui::Key::Escape));
    let frame = Frame::new()
        .fill(palette.overlay)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(CornerRadius::same(theme::RADIUS + 4))
        .inner_margin(Margin::same(24))
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 40,
            spread: 0,
            color: palette.shadow,
        });
    egui::Window::new(title.as_ref())
        .id(egui::Id::new("spotifast-update"))
        .title_bar(false)
        .resizable(false)
        .auto_sized()
        .frame(frame)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(420.0_f32.min((ctx.content_rect().width() - 64.0).max(240.0)));
            ui.horizontal(|ui| {
                theme::text(ui, title.as_ref(), theme::bold(20.0), palette.text);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close |= theme::icon_button(
                        ui,
                        Icon::X,
                        18.0,
                        palette.secondary,
                        palette.text,
                        &gettext(locale, "Close update"),
                    )
                    .clicked();
                });
            });
            ui.add_space(4.0);
            theme::text(
                ui,
                format!(
                    "{} → {}",
                    env!("CARGO_PKG_VERSION"),
                    app.fork_update
                        .as_ref()
                        .map(|client| client.status.version.clone())
                        .filter(|version| !version.is_empty())
                        .unwrap_or_else(|| release.version.to_string())
                ),
                theme::regular(14.0),
                palette.secondary,
            );
            ui.add_space(20.0);
            if app.fork_update.is_some() {
                show_fork_progress(app, ui, release.url.as_str());
                return;
            }
            let mut action = None;
            let mut release_link = gettext(locale, "Release notes");
            match &app.update_download {
                DownloadState::Downloading { received, total } => {
                    let checking = *total > 0 && received == total;
                    theme::text(
                        ui,
                        if checking {
                            gettext(locale, "Checking download…")
                        } else {
                            gettext(locale, "Downloading update…")
                        },
                        theme::medium(14.0),
                        palette.text,
                    );
                    ui.add_space(8.0);
                    if *total > 0 {
                        ui.add(
                            egui::ProgressBar::new(*received as f32 / *total as f32)
                                .fill(palette.accent)
                                .desired_height(6.0),
                        );
                        ui.add_space(6.0);
                        theme::text(
                            ui,
                            // Translators: {received} and {total} are sizes in megabytes, such as 12.5.
                            gettext(locale, "{received} of {total} MB")
                                .replace(
                                    "{received}",
                                    &format!("{:.1}", *received as f64 / 1_000_000.0),
                                )
                                .replace("{total}", &format!("{:.1}", *total as f64 / 1_000_000.0)),
                            theme::regular(12.0),
                            palette.secondary,
                        );
                    } else {
                        theme::spinner(ui, 16.0, palette.accent);
                    }
                }
                DownloadState::Ready(_) => {
                    theme::text(
                        ui,
                        gettext(locale, "Ready to install"),
                        theme::semibold(14.0),
                        palette.text,
                    );
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(gettext(
                                locale,
                                "Music playing on this computer will stop when Spotifast restarts.",
                            ))
                            .font(theme::regular(14.0))
                            .color(palette.secondary),
                        )
                        .wrap(),
                    );
                    action = Some((gettext(locale, "Restart to update"), Action::InstallUpdate));
                }
                DownloadState::Installing => {
                    ui.horizontal(|ui| {
                        theme::spinner(ui, 16.0, palette.accent);
                        theme::text(
                            ui,
                            gettext(locale, "Preparing to restart…"),
                            theme::regular(14.0),
                            palette.text,
                        );
                    });
                }
                DownloadState::Idle | DownloadState::Failed(_) => {
                    if let DownloadState::Failed(error) = &app.update_download {
                        ui.add(
                            egui::Label::new(
                                RichText::new(error)
                                    .font(theme::regular(14.0))
                                    .color(palette.danger),
                            )
                            .wrap(),
                        );
                        ui.add_space(8.0);
                    }
                    match &app.update_support {
                        None => {
                            theme::spinner(ui, 16.0, palette.accent);
                        }
                        Some(Err(reason)) => {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(reason)
                                        .font(theme::regular(14.0))
                                        .color(palette.secondary),
                                )
                                .wrap(),
                            );
                            release_link = gettext(locale, "Download from GitHub");
                        }
                        Some(Ok(_)) => {
                            action = Some((
                                if matches!(app.update_download, DownloadState::Failed(_)) {
                                    gettext(locale, "Retry download")
                                } else {
                                    gettext(locale, "Download update")
                                },
                                Action::DownloadUpdate,
                            ));
                        }
                    }
                }
            }
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some((label, action)) = action
                        && theme::pill_button(ui, &palette, &label, true).clicked()
                    {
                        app.actions.push(action);
                    }
                    ui.add_space(8.0);
                    ui.add(egui::Hyperlink::from_label_and_url(
                        RichText::new(release_link)
                            .font(theme::medium(13.0))
                            .color(palette.secondary),
                        if app.update_source.is_github()
                            && matches!(app.update_support, Some(Err(_)))
                        {
                            "https://github.com/marsa099/spotifast"
                        } else {
                            release.url.as_str()
                        },
                    ));
                });
            });
        });
    if close {
        app.show_update = false;
    }
}

fn fork_note(ui: &mut egui::Ui, palette: &theme::Palette, text: &str) {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .font(theme::regular(14.0))
                .color(palette.secondary),
        )
        .wrap(),
    );
}

fn show_fork_progress(app: &mut App, ui: &mut egui::Ui, release_url: &str) {
    let client = app.fork_update.as_ref().expect("managed fork updater");
    let status = client.status.clone();
    let pending = client.pending;
    let palette = app.palette;
    let locale = app.locale;
    let mut action = None;
    let mut attach = false;
    if status.phase == "needs_input" {
        attach |= theme::soft_button(
            ui,
            &palette,
            Some(Icon::ExternalLink),
            &gettext(locale, "Needs your input"),
            true,
        )
        .clicked();
    } else if pending && status.ready() {
        theme::text(
            ui,
            gettext(locale, "Preparing to restart…"),
            theme::semibold(14.0),
            palette.text,
        );
    } else if status.ready() {
        theme::text(
            ui,
            gettext(locale, "Ready to install"),
            theme::semibold(14.0),
            palette.text,
        );
        action = Some((gettext(locale, "Restart to update"), Action::InstallUpdate));
        ui.add_space(6.0);
        ui.add(
            egui::Label::new(
                RichText::new(gettext(
                    locale,
                    "Music playing on this computer will stop when Spotifast restarts.",
                ))
                .font(theme::regular(14.0))
                .color(palette.secondary),
            )
            .wrap(),
        );
    } else if status.phase == "failed" || status.phase == "idle" {
        let estimate = match status.last_minutes() {
            Some(minutes) => gettext(locale, "The last update took about {minutes} minutes.")
                .replace("{minutes}", &minutes.to_string()),
            None => gettext(
                locale,
                "The first update takes about 30–60 minutes, including a one-time setup. Later updates usually take 15–25 minutes.",
            )
            .into_owned(),
        };
        fork_note(
            ui,
            &palette,
            &format!(
                "{} {}",
                gettext(
                    locale,
                    "Updating rebuilds and tests Spotifast Lab on this computer. It uses up to 8 CPU cores and 8 GB of memory, so the computer may run warm and slow.",
                ),
                estimate
            ),
        );
        ui.add_space(6.0);
        fork_note(
            ui,
            &palette,
            &gettext(locale, "You can keep listening while it runs."),
        );
        action = Some((
            gettext(
                locale,
                if status.phase == "failed" {
                    "Retry download"
                } else {
                    "Download update"
                },
            ),
            Action::DownloadUpdate,
        ));
    } else {
        ui.horizontal(|ui| {
            theme::spinner(ui, 16.0, palette.accent);
            theme::text(
                ui,
                gettext(locale, "Checking fork compatibility…"),
                theme::medium(14.0),
                palette.text,
            );
        });
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let progress = match (status.elapsed_minutes(now), status.remaining_minutes(now)) {
            (Some(elapsed), Some(0)) => Some(
                gettext(locale, "Running for {elapsed} min, longer than last time.")
                    .replace("{elapsed}", &elapsed.to_string()),
            ),
            (Some(elapsed), Some(left)) => Some(
                gettext(locale, "Running for {elapsed} min, about {left} min left.")
                    .replace("{elapsed}", &elapsed.to_string())
                    .replace("{left}", &left.to_string()),
            ),
            (Some(elapsed), None) => Some(
                gettext(locale, "Running for {elapsed} min.")
                    .replace("{elapsed}", &elapsed.to_string()),
            ),
            (None, _) => None,
        };
        ui.add_space(6.0);
        if let Some(progress) = progress {
            fork_note(ui, &palette, &progress);
        }
        fork_note(
            ui,
            &palette,
            &gettext(locale, "You can keep listening while it runs."),
        );
        // Minutes tick without status changes.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(30));
    }
    if !status.message.is_empty() {
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("fork-update-details")
            .max_height(180.0)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(&status.message)
                            .font(theme::regular(14.0))
                            .color(if status.phase == "failed" {
                                palette.danger
                            } else {
                                palette.secondary
                            }),
                    )
                    .wrap(),
                );
            });
    }
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some((label, requested)) = action
                && !pending
                && theme::pill_button(ui, &palette, &label, true).clicked()
            {
                app.actions.push(requested);
            }
            if status.phase != "idle" {
                attach |= theme::soft_button(
                    ui,
                    &palette,
                    Some(Icon::ExternalLink),
                    &gettext(locale, "Open update terminal"),
                    false,
                )
                .clicked();
            }
            ui.add(egui::Hyperlink::from_label_and_url(
                RichText::new(gettext(locale, "Release notes"))
                    .font(theme::medium(13.0))
                    .color(palette.secondary),
                release_url,
            ));
        });
    });
    if attach {
        app.actions.push(Action::OpenUpdateTerminal);
    }
}
