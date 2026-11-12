use crate::error::AppResult;
use std::path::Path;

fn patch_number(name: &str) -> Option<Vec<u64>> {
    name.split('.').map(|part| part.parse().ok()).collect()
}

pub async fn newest_table(root: &Path) -> AppResult<Option<Vec<u8>>> {
    let dir = root.join("winprob");
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut patches = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let table = entry.path().join("fit").join("winprob.toml");
        if let Some(number) = entry.file_name().to_str().and_then(patch_number)
            && tokio::fs::metadata(&table).await.is_ok_and(|m| m.is_file())
        {
            patches.push((number, table));
        }
    }
    let Some((_, table)) = patches.into_iter().max_by(|a, b| a.0.cmp(&b.0)) else { return Ok(None) };
    Ok(Some(tokio::fs::read(table).await?))
}
