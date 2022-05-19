use crate::provider::CloudProvider;
use anyhow::Result;

pub async fn run(provider: &impl CloudProvider) -> Result<()> {
    let (id, size, hash) = provider.upload_file(std::path::Path::new("/tmp/JetBrainsMono-2.242.zip")).await?;

    println!("upload {:?} {:?} {:?}", id, size, hash);

    Ok(())
}
