use crate::error::AppResult;
use std::path::Path;

pub fn patch_number(name: &str) -> Option<Vec<u64>> {
    name.split('.').map(|part| part.parse().ok()).collect()
}

pub async fn newest_table(root: &Path) -> AppResult<Option<Vec<u8>>> {
    newest_file(root, &Path::new("fit").join("winprob.toml")).await
}

pub async fn newest_builds(root: &Path) -> AppResult<Option<Vec<u8>>> {
    newest_file(root, Path::new("builds.json")).await
}

async fn newest_file(root: &Path, name: &Path) -> AppResult<Option<Vec<u8>>> {
    let dir = root.join("winprob");
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut patches = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let file = entry.path().join(name);
        if let Some(number) = entry.file_name().to_str().and_then(patch_number)
            && tokio::fs::metadata(&file).await.is_ok_and(|m| m.is_file())
        {
            patches.push((number, file));
        }
    }
    let Some((_, file)) = patches.into_iter().max_by(|a, b| a.0.cmp(&b.0)) else { return Ok(None) };
    Ok(Some(tokio::fs::read(file).await?))
}
