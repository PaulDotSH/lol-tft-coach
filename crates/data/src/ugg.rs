//! u.gg ARAM: Mayhem data from static.bigbrain.gg.

use model::{ChampAugments, ChampKey, MayhemData};

/// `ddragon_version` like "16.21.1" → u.gg patch "16_21".
pub async fn mayhem(http: &crate::Http, ddragon_version: &str) -> anyhow::Result<MayhemData> {
    let _ = (http, ddragon_version);
    todo!()
}

pub async fn champ_augments(http: &crate::Http, patch: &str, champ: ChampKey) -> anyhow::Result<ChampAugments> {
    let _ = (http, patch, champ);
    todo!()
}
