pub const BUNDLED: &[(&str, &str)] = &[
    ("Aster · Prism", include_str!("../presets/prism.toml")),
    (
        "Aster · Prism Powerline",
        include_str!("../presets/prism-powerline.toml"),
    ),
    ("Aster · Aurora", include_str!("../presets/aurora.toml")),
    (
        "Aster · Aurora Powerline",
        include_str!("../presets/aurora-powerline.toml"),
    ),
    ("Aster · Obsidian", include_str!("../presets/obsidian.toml")),
    (
        "Aster · Midnight Circuit",
        include_str!("../presets/midnight-circuit.toml"),
    ),
    (
        "Aster · Moonstone",
        include_str!("../presets/moonstone.toml"),
    ),
    ("Aster · Ember", include_str!("../presets/ember.toml")),
    (
        "Aster · Deep Forest",
        include_str!("../presets/deep-forest.toml"),
    ),
];

pub fn get(name: &str) -> Option<&'static str> {
    BUNDLED
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, text)| *text)
}
