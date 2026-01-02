use bincode;
use player_settings::PlayerSettings;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

pub fn save_settings<settings: PlayerSettings>(
    path: &Path,
    settings: &settings,
) -> Result<(), std::io::Error> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    bincode::serialize_into(&mut writer, settings)?;
    Ok(())
}

pub fn load_settings(path: &Path) -> Result<PlayerSettings, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let settings = bincode::deserialize_from(&mut reader)?;
    Ok(settings)
}
