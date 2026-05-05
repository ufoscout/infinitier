use infinitier_common::Game;

pub const BG_RESOURCES_DIR: (&str, Game) = ("bg", Game::Bg);
pub const BG_EE_RESOURCES_DIR: (&str, Game) = ("bg_ee", Game::Bgee);
pub const BG2_RESOURCES_DIR: (&str, Game) = ("bg2", Game::Bg2);
pub const BG2_EE_RESOURCES_DIR: (&str, Game) = ("bg2_ee", Game::Bg2ee);
pub const IWD_RESOURCES_DIR: (&str, Game) = ("iwd", Game::Iwd);
pub const IWD_EE_RESOURCES_DIR: (&str, Game) = ("iwd_ee", Game::Iwdee);
pub const IWD2_RESOURCES_DIR: (&str, Game) = ("iwd2", Game::Iwd2);
pub const PST_RESOURCES_DIR: (&str, Game) = ("pst", Game::Pst);
pub const PST_EE_RESOURCES_DIR: (&str, Game) = ("pst_ee", Game::Pstee);

pub const ALL_RESOURCES_DIRS: &[(&str, Game)] = &[
    BG_RESOURCES_DIR,
    BG_EE_RESOURCES_DIR,
    BG2_RESOURCES_DIR,
    BG2_EE_RESOURCES_DIR,
    IWD_RESOURCES_DIR,
    IWD_EE_RESOURCES_DIR,
    IWD2_RESOURCES_DIR,
    PST_RESOURCES_DIR,
    PST_EE_RESOURCES_DIR,
];
