//! Unified last-frame hit targets for mouse picking.

use crate::input::MouseCell;
use crate::rect::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UiHitTarget {
    MainMenuItem(usize),
    DialogueChoice(usize),
    DialogueContinue,
    /// Row index into [`crate::narrative::NarrativeState::inventory`] stacks.
    InventoryStack(usize),
    /// Row index into the quest journal list (left column).
    JournalQuest(usize),
    /// Player inventory column in the transfer overlay.
    TransferPlayerStack(usize),
    /// Container column in the transfer overlay.
    TransferContainerStack(usize),
    /// Level-editor control or modal-picker row (see [`EditorHitTarget`]).
    Editor(EditorHitTarget),
}

/// Clickable controls in the level editor: sidebar rows and modal picker list rows. Lives in the
/// shared UI registry so the editor uses the same [`UiHitState`] pick path as the game shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorHitTarget {
    ClearPropOverlay,
    OpenTerrainPicker,
    OpenEntityPicker,
    LayerGround,
    LayerProp,
    ModePaint,
    ModePlace,
    ModeErase,
    ModePlayer,
    ModeAtmos,
    PlayerSpawnRow,
    /// Visible row index within the active modal picker's list window.
    PickerRow(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiHitState {
    /// Later entries win on overlap (drawn on top).
    regions: Vec<(UiHitTarget, Rect)>,
}

impl UiHitState {
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn push(&mut self, id: UiHitTarget, rect: Rect) {
        self.regions.push((id, rect));
    }

    #[must_use]
    pub fn pick(&self, cell: MouseCell) -> Option<UiHitTarget> {
        self.regions
            .iter()
            .rev()
            .find(|(_, r)| r.contains(cell.x, cell.y))
            .map(|(id, _)| *id)
    }
}
