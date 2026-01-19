macro_rules! requires {
    ($($cond:expr),* $(,)?) => {
        &[$($cond),*]
    };
}

macro_rules! effects {
    ($($eff:expr),* $(,)?) => {
        &[$($eff),*]
    };
}

macro_rules! quest_defs {
    ($array_name:ident; $($const_name:ident => ($id:literal, $title:literal)),+ $(,)?) => {
        $(pub const $const_name: &str = $id;)+
        pub static $array_name: &[crate::content::QuestDef] = &[
            $(crate::content::QuestDef { id: $id, title: $title }),+
        ];
    };
}

macro_rules! dialogue_tree {
    (
        $tree_name:ident, $dialogue_id:literal, {
            $(
                $node:ident => {
                    text: $text:expr,
                    $(text_fn: $text_fn:expr,)?
                    $(effects: $node_effects:expr,)?
                    choices: [
                        $(
                            {
                                label: $label:expr,
                                next: $next:ident,
                                requires: $requires:expr,
                                $(requires_fn: $requires_fn:expr,)?
                                effects: $effects:expr,
                                $(effects_fn: $effects_fn:expr,)?
                            }
                        ),* $(,)?
                    ],
                    $(continue_to: $continue_to:ident,)?
                }
            ),+ $(,)?
        }
    ) => {
        #[allow(non_camel_case_types, dead_code)]
        enum __Node {
            $($node),+
        }

        const __EXIT: usize = dialogue_tree!(@count $($node),+);

        static __NODES: &[crate::content::DialogueNode] = &[
            $(
                crate::content::DialogueNode {
                    id: stringify!($node),
                    text: $text,
                    text_fn: dialogue_tree!(@opt_text_fn $($text_fn)?),
                    effects: dialogue_tree!(@opt_node_effects $($node_effects)?),
                    auto_next: dialogue_tree!(@opt_continue __Node, __EXIT $(, $continue_to)?),
                    choices: &[
                        $(
                            crate::content::DialogueChoice {
                                label: $label,
                                next: dialogue_tree!(@next __Node, __EXIT, $next),
                                requires: $requires,
                                requires_fn: dialogue_tree!(@opt_requires_fn $($requires_fn)?),
                                effects: $effects,
                                effects_fn: dialogue_tree!(@opt_effects_fn $($effects_fn)?),
                            }
                        ),*
                    ],
                }
            ),+
        ];

        pub static $tree_name: crate::content::DialogueTree = crate::content::DialogueTree {
            id: $dialogue_id,
            nodes: __NODES,
        };
    };

    (@next $enum_name:ident, $exit:ident, EXIT) => {
        $exit
    };
    (@next $enum_name:ident, $exit:ident, $next:ident) => {
        $enum_name::$next as usize
    };
    (@opt_text_fn $text_fn:expr) => {
        Some($text_fn)
    };
    (@opt_text_fn) => {
        None
    };
    (@opt_node_effects $effects:expr) => {
        $effects
    };
    (@opt_node_effects) => {
        &[]
    };
    (@opt_continue $enum_name:ident, $exit:ident , $continue:ident) => {
        Some(dialogue_tree!(@next $enum_name, $exit, $continue))
    };
    (@opt_continue $enum_name:ident, $exit:ident) => {
        None
    };
    (@opt_requires_fn $requires_fn:expr) => {
        Some($requires_fn)
    };
    (@opt_requires_fn) => {
        None
    };
    (@opt_effects_fn $effects_fn:expr) => {
        Some($effects_fn)
    };
    (@opt_effects_fn) => {
        None
    };
    (@count $($nodes:ident),+ $(,)?) => {
        <[()]>::len(&[$(dialogue_tree!(@replace $nodes ())),+])
    };
    (@replace $_t:tt $sub:expr) => {
        $sub
    };
}

pub(crate) use dialogue_tree;
pub(crate) use effects;
pub(crate) use quest_defs;
pub(crate) use requires;
