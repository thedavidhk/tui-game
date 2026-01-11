use crate::game_content::{dialogue_tree, effects, requires};

dialogue_tree! {
    TREE_MERCHANT, "merchant", {
        hub => {
            text: "(spreads empty hands) Pins, twine, charms - cleaned out till the next caravan rolls in.",
            choices: [
                {
                    label: "Farewell.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        }
    }
}
