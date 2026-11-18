//! The menu style option: the Bedrock look or the Java look for the title, pause and Options
//! screens. The same menus and settings back both; only how they are drawn changes.

use super::{SettingId, SettingsOptions, definitions::SettingChoice};

/// How the menus are drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MenuStyle {
    /// Bedrock's Ore UI look.
    #[default]
    Bedrock,
    /// The Java Edition look, built from the player's own Java Edition files.
    Java,
}

impl MenuStyle {
    /// Every style in the order the option shows them; each index is the saved value.
    pub const ALL: [Self; 2] = [Self::Bedrock, Self::Java];

    /// The saved value of this style.
    #[must_use]
    pub const fn value(self) -> i32 {
        match self {
            Self::Bedrock => 0,
            Self::Java => 1,
        }
    }

    /// The other style, for the toggle buttons.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Bedrock => Self::Java,
            Self::Java => Self::Bedrock,
        }
    }

    /// The style's name as the menus show it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Bedrock => "Bedrock",
            Self::Java => "Java",
        }
    }
}

/// The option's choices, one per [`MenuStyle`] in saved-value order.
pub const CHOICES: &[SettingChoice] = &[
    SettingChoice {
        name: "menu_style_bedrock",
        label: MenuStyle::Bedrock.label(),
    },
    SettingChoice {
        name: "menu_style_java",
        label: MenuStyle::Java.label(),
    },
];

/// The saved value of the default style.
pub const DEFAULT: i32 = MenuStyle::Bedrock.value();

impl SettingsOptions {
    /// The selected menu style; an unknown saved value keeps the default.
    #[must_use]
    pub fn menu_style(&self) -> MenuStyle {
        usize::try_from(self.value(SettingId::MenuStyle))
            .ok()
            .and_then(|index| MenuStyle::ALL.get(index).copied())
            .unwrap_or_default()
    }

    /// The menu action's setting index and value that switch to the other style.
    #[must_use]
    pub fn menu_style_switch(&self) -> (u16, i32) {
        (
            u16::try_from(SettingId::MenuStyle.index()).unwrap_or(u16::MAX),
            self.menu_style().other().value(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const _: () = assert!(CHOICES.len() == MenuStyle::ALL.len());

    #[test]
    fn every_style_has_one_choice_at_its_saved_value() {
        for style in MenuStyle::ALL {
            assert_eq!(MenuStyle::ALL[style.value() as usize], style);
            assert_eq!(CHOICES[style.value() as usize].label, style.label());
            assert_ne!(style.other(), style);
            assert_eq!(style.other().other(), style);
        }
    }

    #[test]
    fn menus_start_in_the_bedrock_style_and_the_choice_survives_a_save() {
        let mut options = SettingsOptions::default();
        assert_eq!(options.menu_style(), MenuStyle::Bedrock);
        let (index, value) = options.menu_style_switch();
        assert_eq!(usize::from(index), SettingId::MenuStyle.index());
        assert!(options.set(usize::from(index), value));
        let restored = SettingsOptions::decode(&serde_json::to_vec(&options).unwrap()).unwrap();
        assert_eq!(restored.menu_style(), MenuStyle::Java);
        assert_eq!(restored.menu_style_switch().1, MenuStyle::Bedrock.value());
    }

    #[test]
    fn a_value_past_the_last_style_is_kept_within_the_choices() {
        let mut options = SettingsOptions::default();
        options.set(SettingId::MenuStyle.index(), 7);
        assert_eq!(options.menu_style(), MenuStyle::Java);
    }
}
