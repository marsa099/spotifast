//! Spatial keyboard selection for central cards, including horizontal shelves.
//! Views publish geometry; App resolves navigation after drawing.

use super::navigation::{Command, Pane};
use crate::{
    app::App,
    model::{Action, Page},
};
use egui::{Id, Rect};

#[derive(Clone, Debug)]
pub struct Card {
    pub id: Id,
    pub page: Page,
    pub rect: Rect,
    pub response: Option<Id>,
    pub row: Option<Box<TrackTarget>>,
}

#[derive(Clone, Debug)]
pub struct TrackTarget {
    pub item: crate::api::models::PlayableItem,
    pub context: crate::model::RowContext,
    pub index: usize,
}

#[derive(Default)]
pub struct GridNavigation {
    pub page: Option<Page>,
    pub cards: Vec<Card>,
    pub selected: Option<Id>,
    pub active: bool,
    pub has_rows: bool,
    pub scroll: Option<Id>,
    pub viewport_height: f32,
}

impl GridNavigation {
    pub fn active_on(&self, page: &Page) -> bool {
        self.page.as_ref() == Some(page) && self.active && !self.cards.is_empty()
    }

    pub fn selected_row(&self) -> Option<&TrackTarget> {
        self.cards
            .iter()
            .find(|card| Some(card.id) == self.selected)?
            .row
            .as_deref()
    }

    pub fn at_left_edge(&self) -> bool {
        self.selected
            .and_then(|id| self.cards.iter().position(|card| card.id == id))
            .is_none_or(|index| neighbor(&self.cards, index, Command::Left).is_none())
    }

    /// Install a complete frame before resolving its keys. Updating geometry
    /// first prevents activation of a card removed by filtering or a response.
    pub fn prepare(&mut self, actions: &[Action]) {
        let Some(page) = actions.iter().find_map(|action| match action {
            Action::GridFrame(page) => Some(page),
            _ => None,
        }) else {
            return;
        };
        let same_page = self.page.as_ref() == Some(page);
        let has_rows = actions
            .iter()
            .any(|action| matches!(action, Action::GridRows));
        let mut cards: Vec<Card> = Vec::new();
        let mut indices = std::collections::HashMap::new();
        for action in actions {
            if let Action::GridCard(card) = action {
                if let Some(index) = indices.get(&card.id).copied() {
                    cards[index] = card.clone();
                } else {
                    indices.insert(card.id, cards.len());
                    cards.push(card.clone());
                }
            }
        }
        self.viewport_height = actions
            .iter()
            .find_map(|action| match action {
                Action::GridViewport(rect) => Some(rect.height()),
                _ => None,
            })
            .unwrap_or(0.0);
        if same_page && self.has_rows && !has_rows {
            self.active = true;
        }
        if !same_page {
            self.selected = None;
            self.scroll = None;
            self.active = !has_rows;
        }
        if self.selected.is_some_and(|id| !indices.contains_key(&id)) {
            self.selected = None;
            self.scroll = None;
        }
        self.has_rows = has_rows;
        self.page = Some(page.clone());
        self.cards = cards;
    }

    pub fn focus(&mut self) {
        if let Some(card) = self.cards.first() {
            self.active = true;
            self.selected = Some(card.id);
            self.scroll = self.selected;
        }
    }

    pub fn navigate(&mut self, command: Command) -> Option<Action> {
        let current = self
            .selected
            .and_then(|id| self.cards.iter().position(|card| card.id == id));
        if matches!(command, Command::Play | Command::Open) {
            let card = self.cards.get(current?)?;
            if let Some(row) = &card.row {
                return match command {
                    Command::Play if super::navigation::can_play(&row.item) => {
                        Some(Action::PlayFromRow {
                            context: row.context.clone(),
                            uri: row.item.uri().to_string(),
                            index: match &row.context {
                                crate::model::RowContext::Uris(uris) if uris.len() == 1 => 0,
                                _ => row.index as u32,
                            },
                        })
                    }
                    Command::Open => super::navigation::album(&row.item),
                    _ => None,
                };
            }
            return Some(Action::Open(card.page.clone()));
        }
        if command == Command::Clear {
            self.selected = None;
            self.scroll = None;
            return None;
        }
        let next = match command {
            Command::First => (!self.cards.is_empty()).then_some(0),
            Command::Last => self.cards.len().checked_sub(1),
            Command::HalfUp | Command::HalfDown => {
                let mut next = current.or_else(|| (!self.cards.is_empty()).then_some(0));
                if let Some(start) = next {
                    while let Some(candidate) =
                        next.and_then(|index| neighbor(&self.cards, index, command))
                    {
                        next = Some(candidate);
                        if (self.cards[candidate].rect.center().y
                            - self.cards[start].rect.center().y)
                            .abs()
                            >= (self.viewport_height / 2.0).max(1.0)
                        {
                            break;
                        }
                    }
                }
                next
            }
            Command::Left | Command::Right | Command::Up | Command::Down => current.map_or_else(
                || (!self.cards.is_empty()).then_some(0),
                |index| {
                    let next = neighbor(&self.cards, index, command);
                    if next.is_none() && command == Command::Up && self.has_rows {
                        self.active = false;
                    }
                    next.or(Some(index))
                },
            ),
            _ => current,
        };
        self.selected = next.map(|index| self.cards[index].id);
        self.scroll = self.selected;
        None
    }
}

/// Prefer candidates aligned on the movement axis, then the nearest card.
/// Measured rectangles make wrapped rows and differently sized shelves work
/// without assuming a fixed number of columns across the whole page.
pub fn neighbor(cards: &[Card], current: usize, command: Command) -> Option<usize> {
    let from = cards.get(current)?.rect;
    let horizontal = matches!(command, Command::Left | Command::Right);
    let forward = matches!(command, Command::Right | Command::Down | Command::HalfDown);
    cards
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != current)
        .filter_map(|(index, card)| {
            let delta = card.rect.center() - from.center();
            let (along, across, aligned) = if horizontal {
                (
                    delta.x,
                    delta.y.abs(),
                    card.rect.y_range().intersects(from.y_range()),
                )
            } else {
                (
                    delta.y,
                    delta.x.abs(),
                    card.rect.x_range().intersects(from.x_range()),
                )
            };
            let distance = if forward { along } else { -along };
            (distance > 1.0 && (!horizontal || aligned))
                .then_some((index, !aligned, distance, across))
        })
        .min_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| (a.2 + a.3).total_cmp(&(b.2 + b.3)))
        })
        .map(|(index, _, _, _)| index)
}

pub fn card(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, page: Page) {
    if !app.settings.vim_keys {
        return;
    }
    let id = ui.id().with(&page);
    register(app, ui, response, page, id);
}

pub fn card_key(
    app: &mut App,
    ui: &mut egui::Ui,
    response: &egui::Response,
    page: Page,
    key: &str,
) {
    if !app.settings.vim_keys {
        return;
    }
    let id = ui.id().with(key);
    register(app, ui, response, page, id);
}

fn virtual_id(owner: &Page, page: &Page) -> Id {
    Id::new(("library-grid-card", owner, page))
}

pub fn virtual_card(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, page: Page) {
    if !app.settings.vim_keys {
        return;
    }
    let id = virtual_id(app.page(), &page);
    register(app, ui, response, page, id);
}

fn register(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, page: Page, id: Id) {
    app.actions.push(Action::GridCard(Card {
        id,
        page,
        rect: response.rect,
        response: Some(response.id),
        row: None,
    }));
    if app.grid_navigation.active_on(app.page())
        && app
            .navigation
            .active_pane(app.settings.sidebar_visible, app.show_queue_panel)
            == Pane::Main
        && app.grid_navigation.selected == Some(id)
    {
        super::navigation::outline(ui, response.rect);
        if app.grid_navigation.scroll == Some(id) {
            // The enclosing horizontal shelf handles its own x scroll. The
            // page repeats the request after all nested scroll areas close.
            response.scroll_to_me(None);
        }
    }
}

/// Mixed pages interleave cards and track sections. Publish rows in the same
/// geometry stream so movement follows the actual display order.
pub fn track(app: &mut App, response: &egui::Response, id: Id, target: TrackTarget) {
    if !app.settings.vim_keys {
        return;
    }
    app.actions.push(Action::GridCard(Card {
        id,
        page: app.page().clone(),
        rect: response.rect,
        response: Some(response.id),
        row: Some(Box::new(target)),
    }));
}

/// Publish offscreen cards too, while keeping their actual rendering virtual.
/// Uses widgets::virtual_wrapped_cards geometry, with item IDs independent of
/// egui's virtual row scopes and the number of columns after a resize.
pub fn virtual_cards(app: &mut App, ui: &mut egui::Ui, pages: &[Page], height: f32) {
    if !app.settings.vim_keys {
        return;
    }
    let gap = super::widgets::CARD_GAP / 2.0;
    let width = ui.available_width().max(super::widgets::CARD_WIDTH);
    let columns = ((width + gap) / (super::widgets::CARD_WIDTH + gap))
        .floor()
        .max(1.0) as usize;
    let origin = ui.cursor().min;
    for (index, page) in pages.iter().enumerate() {
        let row = index / columns;
        let column = index % columns;
        let id = virtual_id(app.page(), page);
        let rect = Rect::from_min_size(
            origin
                + egui::vec2(
                    column as f32 * (super::widgets::CARD_WIDTH + gap),
                    row as f32 * (height + super::widgets::CARD_GAP),
                ),
            egui::vec2(super::widgets::CARD_WIDTH, height),
        );
        app.actions.push(Action::GridCard(Card {
            id,
            page: page.clone(),
            rect,
            response: None,
            row: None,
        }));
    }
}

/// Nested egui ScrollAreas consume both axes, including disabled axes. A card
/// inside a horizontal shelf therefore cannot scroll the outer page itself.
/// Repeat its current-frame rectangle at the outer vertical scroll boundary.
pub fn scroll_page(app: &mut App, ui: &mut egui::Ui) {
    if !app.settings.vim_keys
        || !app.grid_navigation.active_on(app.page())
        || app
            .navigation
            .active_pane(app.settings.sidebar_visible, app.show_queue_panel)
            != Pane::Main
    {
        return;
    }
    let Some(id) = app.grid_navigation.scroll else {
        return;
    };
    if let Some(rect) = app.actions.iter().rev().find_map(|action| match action {
        Action::GridCard(card) if card.id == id => Some(card.rect),
        _ => None,
    }) {
        ui.scroll_to_rect(rect, None);
        app.actions.push(Action::GridScrolled(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards() -> Vec<Card> {
        (0..18)
            .map(|index| Card {
                id: Id::new(index),
                page: Page::Album(index.to_string()),
                rect: Rect::from_min_size(
                    egui::pos2((index % 3) as f32 * 100.0, (index / 3) as f32 * 100.0),
                    egui::vec2(90.0, 90.0),
                ),
                response: None,
                row: None,
            })
            .collect()
    }

    #[test]
    fn spatial_edges_do_not_wrap_and_half_pages_use_viewport_height() {
        let cards = cards();
        assert_eq!(neighbor(&cards, 0, Command::Right), Some(1));
        assert_eq!(neighbor(&cards, 0, Command::Down), Some(3));
        assert_eq!(neighbor(&cards, 2, Command::Right), None);
        assert_eq!(neighbor(&cards, 3, Command::Left), None);
        let mut grid = GridNavigation {
            selected: Some(cards[0].id),
            cards,
            viewport_height: 500.0,
            ..Default::default()
        };
        grid.navigate(Command::HalfDown);
        assert_eq!(grid.selected, Some(Id::new(9)));
        grid.navigate(Command::HalfUp);
        assert_eq!(grid.selected, Some(Id::new(0)));
    }

    #[test]
    fn filtering_rebuilds_targets_before_activation_and_switches_out_of_rows() {
        let cards = cards();
        let mut grid = GridNavigation {
            page: Some(Page::Search),
            selected: Some(cards[0].id),
            has_rows: true,
            ..Default::default()
        };
        grid.prepare(&[
            Action::GridFrame(Page::Search),
            Action::GridCard(cards[1].clone()),
        ]);
        assert!(grid.active);
        assert!(grid.selected.is_none());
        assert!(grid.navigate(Command::Open).is_none());
        grid.navigate(Command::Down);
        assert!(
            matches!(grid.navigate(Command::Play), Some(Action::Open(page)) if page == cards[1].page)
        );
    }
}
