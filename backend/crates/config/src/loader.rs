use crate::{AppConfig, Error, Result};
use figment::{
    Figment,
    providers::{Env, Format, Yaml},
};

impl AppConfig {
    pub fn load() -> Result<Self> {
        let local_path = std::path::Path::new("config/local.yaml");
        let mut figment = Figment::new().merge(Yaml::file("config/default.yaml"));

        if local_path.exists() {
            figment = figment.merge(Yaml::file(local_path));
        }

        figment
            .merge(Env::prefixed("APP_").split("__"))
            .extract()
            .map_err(|e| Error::Load {
                source: Box::new(e),
            })
    }
}
