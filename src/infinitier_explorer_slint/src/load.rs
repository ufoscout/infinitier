//! Bootstrap: CLI → GameData. No save-game logic; the explorer just
//! needs the KEY/BIFs and any override directory listings.

use infinitier_core::fs::CaseInsensitiveFS;
use infinitier_core::game::GameDataBuilder;
use infinitier_core::game_detect::detect_game;

use crate::Args;
use crate::state::AppState;

pub fn load(args: &Args) -> std::io::Result<AppState> {
    let game = detect_game(&CaseInsensitiveFS::new(args.game_path.as_slice())?)
        .ok_or_else(|| std::io::Error::other("could not detect game type"))?;

    let game_data = GameDataBuilder::new(args.game_path.as_slice(), game)?.build()?;

    Ok(AppState::new(game_data))
}
