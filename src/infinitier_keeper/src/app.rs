use eframe::egui;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::cre::{Cre, Item, ItemFlags, SpellType};

use crate::components::editable_fields::KeeperEditors;
use crate::components::party_selector::PartySelector;
use crate::state::AppState;
use crate::ui::{
    CharacterPanel, HeaderPanel, ItemBrowser, LoadAction, SaveAction, SaveTabStrip, SpellAssign,
    SpellBrowser, inventory_assign_target, inventory_take_browse_request, spell_take_browse_request,
};

pub struct KeeperApp {
    state: AppState,
    header_panel: HeaderPanel,
    save_tab_strip: SaveTabStrip,
    party_selector: PartySelector,
    character_panel: CharacterPanel,
    /// In-flight text buffers for every editable row on the
    /// abilities tab + the Attacks dropdown index. Mirrors the GPUI
    /// keeper's `KeeperEditors`, just without InputState entities —
    /// egui's immediate-mode model collapses the rebind + commit
    /// plumbing to plain owned `String`s held in a map.
    editors: KeeperEditors,
    /// Save-button + confirmation-dialog state. Shown when the user
    /// clicks the header's Save button.
    save_action: SaveAction,
    /// Load-button picker: the "Open Single Player Saved Game" modal.
    load_action: LoadAction,
    /// The floating Item Browser window (all its state + rendering).
    item_browser: ItemBrowser,
    /// The floating Spell Browser window (all its state + rendering).
    spell_browser: SpellBrowser,
    /// Whether the floating "Items" browser window is open. Toggled by
    /// the header's Items button; the window's own close button clears it.
    items_window_open: bool,
    /// Whether the floating "Spells" browser window is open. Toggled by
    /// the header's Spells button; the window's own close button clears it.
    spells_window_open: bool,
}

impl KeeperApp {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            header_panel: HeaderPanel,
            save_tab_strip: SaveTabStrip,
            party_selector: PartySelector::new(),
            character_panel: CharacterPanel,
            editors: KeeperEditors::new(),
            save_action: SaveAction::new(),
            load_action: LoadAction::new(),
            item_browser: ItemBrowser::new(),
            spell_browser: SpellBrowser::new(),
            items_window_open: false,
            spells_window_open: false,
        }
    }
}

impl KeeperApp {
    /// Paint the movable / resizable / closable "Items" and "Spells"
    /// browser windows when their header toggles are on. The Item Browser
    /// can assign its selected item into the inventory slot the Inventory
    /// tab has selected — its "Add to Inventory" button / double-click
    /// return the resref to assign, which we write here.
    fn show_tool_windows(&mut self, ctx: &egui::Context) {
        // Double-clicking a filled inventory slot reveals that item here:
        // open the browser (if closed) and select it.
        if let Some(resref) = inventory_take_browse_request(ctx) {
            self.items_window_open = true;
            self.item_browser.select(resref);
        }
        // Likewise, double-clicking a Spells-tab row reveals that spell in the
        // Spell Browser.
        if let Some(resref) = spell_take_browse_request(ctx) {
            self.spells_window_open = true;
            self.spell_browser.select(resref);
        }

        // The selected member names both browsers' add buttons.
        let member_name = self.selected_member_name();

        // Item browser: the Inventory tab owns the selected-slot state; ask it
        // whether (and where) the browser's item can be assigned. `Some(slot)`
        // enables the add button / double-click and names the target.
        let target = inventory_assign_target(ctx, &self.state);
        let item_label = match &member_name {
            Some(name) => format!("Add to {name} inventory"),
            None => "Add to inventory".to_string(),
        };
        let assign = self.item_browser.show(
            ctx,
            &mut self.items_window_open,
            &self.state.game_data,
            target.is_some(),
            &item_label,
        );
        if let (Some(resref), Some(slot)) = (assign, target) {
            self.assign_to_inventory(slot, &resref);
        }

        // Spell browser: add the selected spell to the current character.
        // AD&D adds to the one spellbook the SPL implies; IWD2 lets the user
        // pick the book from the menu (the browser returns the resolved one).
        let can_add_spell = self.can_add_spell();
        let spell_label = match &member_name {
            Some(name) => format!("Add to {name}"),
            None => "Add to character".to_string(),
        };
        let add_spell = self.spell_browser.show(
            ctx,
            &mut self.spells_window_open,
            &self.state.game_data,
            can_add_spell,
            &spell_label,
        );
        if let Some(assign) = add_spell {
            self.add_spell_to_character(assign);
        }
    }

    /// The selected party member's display name, or `None` when no save is
    /// open or the slot has no name.
    fn selected_member_name(&self) -> Option<String> {
        if self.state.tabs.is_empty() {
            return None;
        }
        let tab = self.state.active();
        let member = tab.save.party_npcs.get(tab.selected_party_index?)?;
        (!member.display_name.is_empty()).then(|| member.display_name.clone())
    }

    /// Whether the selected party member can take a spell from the browser:
    /// an embedded CRE is selected. Both spell models are supported — AD&D
    /// `known_spells` and IWD2's per-book list-2DA blocks.
    fn can_add_spell(&self) -> bool {
        if self.state.tabs.is_empty() {
            return false;
        }
        let tab = self.state.active();
        tab.selected_party_index
            .and_then(|idx| tab.save.party_npcs.get(idx))
            .is_some_and(|m| matches!(m.cre, Some(NpcCre::Cre(_))))
    }

    /// Add a spell to the selected member as a fresh, un-memorised entry. For
    /// AD&D the spellbook/level come from the SPL header; for IWD2 the browser
    /// already resolved the exact book/level/list-index the user picked. No-op
    /// when the selection is invalid, the SPL can't be loaded, or it's already
    /// known.
    fn add_spell_to_character(&mut self, assign: SpellAssign) {
        match assign {
            SpellAssign::Adnd(resref) => {
                // Resolve the book/level from the SPL (immutable borrow) before
                // taking `&mut state` to write the CRE.
                let Some((spell_type, level)) = self.adnd_spell_book(&resref) else {
                    return;
                };
                self.with_selected_cre_mut(|cre| {
                    cre.add_known_spell(spell_type, level, &resref);
                });
            }
            // The browser resolved the book/level/index; add with 0 copies.
            SpellAssign::Iwd2 { book, level, index } => {
                self.with_selected_cre_mut(|cre| {
                    cre.add_iwd2_spell(book, level as usize, index, 0);
                });
            }
        }
    }

    /// The AD&D spellbook type and on-disk (0-based) level for `resref`, read
    /// from its SPL header. `None` if the SPL can't be loaded.
    fn adnd_spell_book(&self, resref: &str) -> Option<(SpellType, u16)> {
        let spl = self.state.game_data.import_spl_by_name(resref).ok()?;
        let spell_type = match spl.header.spell_type() {
            1 => SpellType::Wizard,
            2 => SpellType::Priest,
            // Innate / special / psionic / bard / other → the Innate book.
            _ => SpellType::Innate,
        };
        // SPL levels are 1-based; known spells store them 0-based.
        let level = (spl.header.spell_level() as u16).saturating_sub(1);
        Some((spell_type, level))
    }

    /// Put the item `resref` into inventory `slot` of the selected party
    /// member's CRE, as a single identified copy whose charges/stack the ITM
    /// dictates (`Itm::max_charges`). No-op if the selection is no longer
    /// valid.
    fn assign_to_inventory(&mut self, slot: usize, resref: &str) {
        // The ITM decides the starting quantities (full stack / ability
        // charges); fall back to no charges if it can't be loaded.
        let [quantity1, quantity2, quantity3] = self
            .state
            .game_data
            .import_itm_by_name(resref)
            .map(|itm| itm.max_charges())
            .unwrap_or([0, 0, 0]);
        let item = Item {
            item: resref.to_owned(),
            duration: 0,
            quantity1,
            quantity2,
            quantity3,
            flags: ItemFlags::Identified,
        };
        self.with_selected_cre_mut(|cre| cre.set_inventory_slot_item(slot, item));
    }

    /// Run `edit` against the selected party member's mutable CRE. No-op when
    /// no slot is selected or it isn't an embedded creature.
    fn with_selected_cre_mut(&mut self, edit: impl FnOnce(&mut Cre)) {
        let tab = self.state.active_mut();
        let Some(idx) = tab.selected_party_index else {
            return;
        };
        if let Some(member) = tab.save.party_npcs.get_mut(idx)
            && let Some(NpcCre::Cre(imported)) = member.cre.as_mut()
        {
            edit(imported.cre_mut());
        }
    }
}

impl eframe::App for KeeperApp {
    /// Pre-frame hook (runs once before [`Self::ui`], inside the egui
    /// pass). Works around egui issue https://github.com/emilk/egui/issues/2142: a focused text field only
    /// surrenders keyboard focus on Escape / Tab / arrow-nav or when
    /// another *focusable* widget is clicked — clicking empty space or a
    /// plain label leaves it focused forever, so `Response::lost_focus()`
    /// never fires and the commit-on-blur path in
    /// [`KeeperEditors::show_input`] never runs.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        surrender_focus_on_outside_click(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let has_save = !self.state.tabs.is_empty();
        let header_action = self.header_panel.show(ui);
        if header_action.save_clicked && has_save {
            self.save_action.open(&self.state);
        }
        if header_action.load_clicked {
            self.load_action.open(&mut self.state);
        }
        // Header's Items / Spells buttons toggle their floating windows.
        if header_action.items_clicked {
            self.items_window_open = !self.items_window_open;
        }
        if header_action.spells_clicked {
            self.spells_window_open = !self.spells_window_open;
        }
        self.show_tool_windows(ui.ctx());
        // ── Update phase — everything that can mutate `state` runs
        // first, so the view below reads a settled state in the same
        // frame (no repaint round-trip). The tab strip switches/closes
        // tabs; the modal dialogs (foreground layer, so still painted
        // on top) may open a different save into the active tab.
        self.save_tab_strip.show(ui, &mut self.state);
        self.save_action.show(ui.ctx(), &mut self.state);
        self.load_action.show(ui.ctx(), &mut self.state);

        // ── View phase — render from the now-settled state. `prepare`
        // re-mirrors the editor buffers from whatever save is active, so
        // a switch made above shows immediately.
        if self.state.tabs.is_empty() {
            empty_state(ui);
        } else {
            self.party_selector.prepare(&self.state, ui.ctx());
            self.editors.prepare(&self.state);
            self.party_selector.show(ui, &mut self.state);
            self.character_panel
                .show(ui, &mut self.state, &mut self.editors);
        }
    }
}

/// Placeholder shown when every save tab has been closed, inviting the
/// user to open one via the header's Load button.
fn empty_state(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.heading("No save loaded");
            ui.add_space(6.0);
            ui.label("Use the Load button above to open a saved game.");
        });
    });
}

/// Drop keyboard focus from the currently-focused widget when the latest
/// pointer press landed outside it. See [`KeeperApp::logic`] for why this
/// is needed (egui https://github.com/emilk/egui/issues/2142) and why it has to run at the top of the frame.
///
/// Generic on purpose: any focused widget surrenders focus on an outside
/// click, which matches normal desktop behaviour and means every editable
/// row — present and future — gets correct commit-on-blur for free,
/// without each call site having to opt in.
fn surrender_focus_on_outside_click(ctx: &egui::Context) {
    // Only a fresh pointer press can move focus off a field.
    if !ctx.input(|i| i.pointer.any_pressed()) {
        return;
    }
    let Some(focused) = ctx.memory(|m| m.focused()) else {
        return;
    };
    let Some(press_pos) = ctx.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    // Rect of the focused widget as laid out last frame — it sits in the
    // same place this frame, since `logic` runs before anything is
    // redrawn. A press inside it means the user is interacting with the
    // field itself (clicking to place the caret, selecting text), so leave
    // focus alone; only an outside press counts as a blur.
    let clicked_inside = ctx
        .read_response(focused)
        .is_some_and(|r| r.interact_rect.contains(press_pos));
    if !clicked_inside {
        ctx.memory_mut(|m| m.surrender_focus(focused));
    }
}
