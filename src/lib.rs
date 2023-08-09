use rand::Rng;
use serde::Serialize;
use shuttle_runtime::async_trait;
use shuttle_service::{error::CustomError, Factory, ResourceBuilder, Type};
use std::path::PathBuf;

const DEFAULT_FOLDER: &str = ".env";
const DEFAULT_ENV_PROD: &str = ".env";

#[derive(Serialize)]
pub struct EnvVars<'a> {
    /// The folder to reach at runtime. Defaults to `.env`.
    folder: &'a str,
    /// The name of the file to use in production. Defaults to `.env`.
    env_prod: &'a str,
    /// The name of the file to use in local.
    env_local: Option<&'a str>,
    /// The static provider to use.
    static_provider: Option<shuttle_static_folder::StaticFolder<'a>>,
    /// The path to be use as config.
    /// This should use a random string to avoid caching.
    /// By doing this, we will be able to always load the env vars.
    /// Note that this is temporary. Ideally, we should be able to change the
    /// Output, but due to a current limitation we are using this workaround.
    config: String,
}

#[derive(Debug)]
pub struct EnvError(dotenvy::Error);

impl<'a> EnvVars<'a> {
    #[must_use]
    pub fn folder(mut self, folder: &'a str) -> Self {
        self.folder = folder;
        self.config = Self::get_config(folder);
        self.static_provider = self.static_provider.map(|p| p.folder(folder));
        self
    }

    #[must_use]
    pub const fn env_prod(mut self, env_prod: &'a str) -> Self {
        self.env_prod = env_prod;
        self
    }

    #[must_use]
    pub const fn env_local(mut self, env_local: &'a str) -> Self {
        self.env_local = Some(env_local);
        self
    }

    pub fn load_env_vars(&self, output_dir: Option<&PathBuf>) -> Result<PathBuf, EnvError> {
        let env_path = output_dir.map_or_else(
            || self.env_local.unwrap_or("").into(),
            |dir| dir.join(self.env_prod),
        );

        if env_path.as_os_str().is_empty() {
            tracing::info!(?env_path, "Is empty!");
            return Ok("".into());
        }

        tracing::info!(?env_path, "Loading env vars from file");

        dotenvy::from_filename(env_path).map_err(|e| {
            tracing::error!(?e, "Failed to load env vars");
            EnvError(e)
        })
    }

    fn get_config(folder: &'a str) -> String {
        let mut rng = rand::thread_rng();
        let y: f64 = rng.gen(); // generates a float between 0 and 1
        let result = format!("{} - {y}", folder);
        result
    }
}

#[async_trait]
impl<'a> ResourceBuilder<PathBuf> for EnvVars<'a> {
    const TYPE: Type = Type::StaticFolder;
    type Config = String;
    type Output = PathBuf;

    fn new() -> Self {
        let static_provider = shuttle_static_folder::StaticFolder::new().folder(DEFAULT_FOLDER);
        Self {
            folder: DEFAULT_FOLDER,
            config: DEFAULT_FOLDER.to_string(),
            env_prod: DEFAULT_ENV_PROD,
            env_local: None,
            static_provider: Some(static_provider),
        }
    }

    fn config(&self) -> &String {
        &self.config
    }

    async fn output(
        mut self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        tracing::info!("Calling output function");

        // is production?
        let env = factory.get_environment();
        let is_production = match env {
            shuttle_service::Environment::Production => true,
            shuttle_service::Environment::Local => false,
        };

        tracing::debug!(?is_production, "Is production?");

        if !is_production {
            tracing::info!("Not in production, loading env vars from file");
            return Ok(self.load_env_vars(None)?);
        }

        tracing::trace!("Calling Static provider");
        let static_provider = self
            .static_provider
            .take()
            .expect("Static Provider is missing");
        let output_dir = static_provider.output(factory).await?;
        tracing::info!(?output_dir, "Static provider returned");
        self.load_env_vars(Some(&output_dir))?;
        Ok(output_dir)
    }

    async fn build(build_data: &Self::Output) -> Result<PathBuf, shuttle_service::Error> {
        Ok(build_data.clone())
    }
}

impl From<EnvError> for shuttle_service::Error {
    fn from(error: EnvError) -> Self {
        let msg = format!("Cannot load env vars: {error:?}");
        Self::Custom(CustomError::msg(msg))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use shuttle_runtime::async_trait;
    use shuttle_service::{DatabaseReadyInfo, Factory, ResourceBuilder};
    use tempfile::{Builder, TempDir};

    use super::*;

    struct MockFactory {
        temp_dir: TempDir,
        is_production: bool,
    }

    // Will have this tree across all the production tests
    // .
    // ├── build
    // │   └── .env
    // │       └── .env
    // ├── storage
    // │   └── .env
    // │       └── .env
    // └── escape
    //     └── passwd
    impl MockFactory {
        fn new(is_production: bool) -> Self {
            Self {
                temp_dir: Builder::new().prefix("env_folder").tempdir().unwrap(),
                is_production,
            }
        }

        fn build_path(&self) -> PathBuf {
            self.get_path("build")
        }

        fn storage_path(&self) -> PathBuf {
            self.get_path("storage")
        }

        fn escape_path(&self) -> PathBuf {
            self.get_path("escape")
        }

        fn get_path(&self, folder: &str) -> PathBuf {
            let path = self.temp_dir.path().join(folder);

            if !path.exists() {
                fs::create_dir(&path).unwrap();
            }

            path
        }
    }

    #[async_trait]
    impl Factory for MockFactory {
        async fn get_db_connection(
            &mut self,
            _db_type: shuttle_service::database::Type,
        ) -> Result<DatabaseReadyInfo, shuttle_service::Error> {
            panic!("no env folder test should try to get a db connection string")
        }

        async fn get_secrets(
            &mut self,
        ) -> Result<std::collections::BTreeMap<String, String>, shuttle_service::Error> {
            panic!("no env folder test should try to get secrets")
        }

        fn get_service_name(&self) -> shuttle_service::ServiceName {
            panic!("no env folder test should try to get the service name")
        }

        fn get_environment(&self) -> shuttle_service::Environment {
            if self.is_production {
                shuttle_service::Environment::Production
            } else {
                shuttle_service::Environment::Local
            }
        }

        fn get_build_path(&self) -> Result<std::path::PathBuf, shuttle_service::Error> {
            Ok(self.build_path())
        }

        fn get_storage_path(&self) -> Result<std::path::PathBuf, shuttle_service::Error> {
            Ok(self.storage_path())
        }
    }

    #[tokio::test]
    async fn copies_folder_if_production() {
        let mut factory = MockFactory::new(true);

        const CONTENT: &str = "MY_VAR=1";

        let input_file_path = factory
            .build_path()
            .join(DEFAULT_FOLDER)
            .join(DEFAULT_ENV_PROD);
        fs::create_dir_all(input_file_path.parent().unwrap()).unwrap();
        fs::write(input_file_path, CONTENT).unwrap();

        let expected_file = factory
            .storage_path()
            .join(DEFAULT_FOLDER)
            .join(DEFAULT_ENV_PROD);

        assert!(!expected_file.exists(), "input file should not exist yet");

        // Call plugin
        let env_folder = EnvVars::new();
        let actual_folder = env_folder.output(&mut factory).await.unwrap();

        assert_eq!(
            actual_folder,
            factory.storage_path().join(DEFAULT_FOLDER),
            "expect path to the env folder to be in the storage folder"
        );
        assert!(
            expected_file.exists(),
            "expected input file to be created in storage folder"
        );
        assert_eq!(
            fs::read_to_string(expected_file).unwrap(),
            CONTENT,
            "expected file content to match"
        );
    }

    #[tokio::test]
    async fn copies_folder_if_production_with_custom_folder_and_prod_file() {
        let mut factory = MockFactory::new(true);

        const CONTENT: &str = "MY_VAR=1";
        const ENV_FOLDER: &str = "custom_env_folder";
        const ENV_PROD_FILE: &str = ".env-prod";

        let input_file_path = factory.build_path().join(ENV_FOLDER).join(ENV_PROD_FILE);
        fs::create_dir_all(input_file_path.parent().unwrap()).unwrap();
        fs::write(input_file_path, CONTENT).unwrap();

        let expected_file = factory.storage_path().join(ENV_FOLDER).join(ENV_PROD_FILE);

        assert!(!expected_file.exists(), "input file should not exist yet");

        // Call plugin
        let env_folder = EnvVars::new().folder(ENV_FOLDER).env_prod(ENV_PROD_FILE);
        let actual_folder = env_folder.output(&mut factory).await.unwrap();

        assert_eq!(
            actual_folder,
            factory.storage_path().join(ENV_FOLDER),
            "expect path to the env folder to be in the storage folder"
        );
        assert!(
            expected_file.exists(),
            "expected input file to be created in storage folder"
        );
        assert_eq!(
            fs::read_to_string(expected_file).unwrap(),
            CONTENT,
            "expected file content to match"
        );
    }

    #[tokio::test]
    #[should_panic(expected = "Cannot use an absolute path for a static folder")]
    async fn cannot_use_absolute_path() {
        let mut factory = MockFactory::new(true);
        let env_folder = EnvVars::new();

        let _ = env_folder
            .folder("/etc")
            .output(&mut factory)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn can_use_absolute_path_if_local() {
        let mut factory = MockFactory::new(false);
        let env_folder = EnvVars::new();

        let folder = env_folder
            .folder("/etc")
            .output(&mut factory)
            .await
            .unwrap();

        assert!(folder.as_os_str().is_empty(), "should return empty path");
    }

    #[tokio::test]
    async fn folder_is_ignored_if_local_and_local_file_absolute() {
        let mut factory = MockFactory::new(false);

        const CONTENT: &str = "MY_VAR=1";
        const ENV_FOLDER: &str = "../other";
        const ENV_LOCAL_FILE: &str = ".env-dev";

        let local_env_path = factory.build_path().join(ENV_FOLDER).join(ENV_LOCAL_FILE);
        fs::create_dir_all(&local_env_path.parent().unwrap()).unwrap();
        fs::write(&local_env_path, CONTENT).unwrap();

        // Call plugin
        let env_folder = EnvVars::new();

        let folder = env_folder
            .folder("/etc")
            .env_local(local_env_path.to_str().unwrap())
            .output(&mut factory)
            .await
            .unwrap();

        assert_eq!(folder, local_env_path, "should return local env path");
        assert_eq!(std::env::var("MY_VAR").unwrap(), "1", "should load env var");
    }

    #[tokio::test]
    #[should_panic(expected = "Cannot traverse out of crate for a static folder")]
    async fn cannot_traverse_up() {
        let mut factory = MockFactory::new(true);

        let password_file_path = factory.escape_path().join("passwd");
        fs::create_dir_all(password_file_path.parent().unwrap()).unwrap();
        fs::write(password_file_path, "qwerty").unwrap();

        // Call plugin
        let env_folder = EnvVars::new();

        let _ = env_folder
            .folder("../escape")
            .output(&mut factory)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn can_traverse_up_if_local_and_no_local_file() {
        let mut factory = MockFactory::new(false);

        let password_file_path = factory.escape_path().join("passwd");
        fs::create_dir_all(password_file_path.parent().unwrap()).unwrap();
        fs::write(password_file_path, "qwerty").unwrap();

        // Call plugin
        let env_folder = EnvVars::new();

        let folder = env_folder
            .folder("../escape")
            .output(&mut factory)
            .await
            .unwrap();

        assert!(folder.as_os_str().is_empty(), "should return empty path");
    }

    #[tokio::test]
    async fn folder_is_ignored_if_local_and_local_file() {
        let mut factory = MockFactory::new(false);

        const CONTENT: &str = "MY_VAR=1";
        const ENV_FOLDER: &str = "../other";
        const ENV_LOCAL_FILE: &str = ".env-dev";

        let password_file_path = factory.escape_path().join("passwd");
        fs::create_dir_all(password_file_path.parent().unwrap()).unwrap();
        fs::write(password_file_path, "qwerty").unwrap();

        let local_env_path = factory.build_path().join(ENV_FOLDER).join(ENV_LOCAL_FILE);
        fs::create_dir_all(&local_env_path.parent().unwrap()).unwrap();
        fs::write(&local_env_path, CONTENT).unwrap();

        // Call plugin
        let env_folder = EnvVars::new();

        let folder = env_folder
            .folder("../escape")
            .env_local(local_env_path.to_str().unwrap())
            .output(&mut factory)
            .await
            .unwrap();

        assert_eq!(folder, local_env_path, "should return local env path");
        assert_eq!(std::env::var("MY_VAR").unwrap(), "1", "should load env var");
    }

    #[tokio::test]
    #[should_panic(expected = "Cannot load env vars")]
    async fn panics_if_local_and_local_file_is_not_correct() {
        let mut factory = MockFactory::new(false);

        const CONTENT: &str = "MY_VAR=1";
        const ENV_FOLDER: &str = "../other";
        const ENV_LOCAL_FILE: &str = ".env-dev";

        let local_env_path = factory.build_path().join(ENV_FOLDER).join(ENV_LOCAL_FILE);
        fs::create_dir_all(&local_env_path.parent().unwrap()).unwrap();
        fs::write(&local_env_path, CONTENT).unwrap();

        // Call plugin
        let env_folder = EnvVars::new();

        let _ = env_folder
            .folder("random")
            .env_local("random/.env-dev")
            .output(&mut factory)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn works_if_folder_and_prod_file_custom() {
        let mut factory = MockFactory::new(true);

        const CONTENT: &str = "MY_VAR=1";
        const ENV_FOLDER: &str = "other";
        const ENV_PROD_FILE: &str = ".env-prod";

        let env_path = factory.build_path().join(ENV_FOLDER).join(ENV_PROD_FILE);
        fs::create_dir_all(&env_path.parent().unwrap()).unwrap();
        fs::write(&env_path, CONTENT).unwrap();

        // Call plugin
        let env_folder = EnvVars::new();

        let folder = env_folder
            .folder(ENV_FOLDER)
            .env_prod(ENV_PROD_FILE)
            .output(&mut factory)
            .await
            .unwrap();

        let expected_output_folder = factory.storage_path().join(ENV_FOLDER);

        assert_eq!(
            folder, expected_output_folder,
            "should return storage folder"
        );
        assert_eq!(std::env::var("MY_VAR").unwrap(), "1", "should load env var");
    }

    #[tokio::test]
    async fn works_if_folder_and_prod_file_default() {
        let mut factory = MockFactory::new(true);

        const CONTENT: &str = "MY_VAR=1";

        let env_path = factory
            .build_path()
            .join(DEFAULT_FOLDER)
            .join(DEFAULT_ENV_PROD);
        fs::create_dir_all(&env_path.parent().unwrap()).unwrap();
        fs::write(&env_path, CONTENT).unwrap();

        // Call plugin
        let env_folder = EnvVars::new();

        let folder = env_folder
            .folder(DEFAULT_FOLDER)
            .env_prod(DEFAULT_ENV_PROD)
            .output(&mut factory)
            .await
            .unwrap();

        let expected_output_folder = factory.storage_path().join(DEFAULT_FOLDER);

        assert_eq!(
            folder, expected_output_folder,
            "should return storage folder"
        );
        assert_eq!(std::env::var("MY_VAR").unwrap(), "1", "should load env var");
    }

    #[tokio::test]
    #[should_panic(expected = "Cannot load env vars")]
    async fn panics_if_folder_and_prod_file_default_not_present() {
        let mut factory = MockFactory::new(true);

        let env_path = factory
            .build_path()
            .join(DEFAULT_FOLDER)
            .join(DEFAULT_ENV_PROD);
        fs::create_dir_all(&env_path.parent().unwrap()).unwrap();

        // Call plugin
        let env_folder = EnvVars::new();

        let _ = env_folder
            .folder(DEFAULT_FOLDER)
            .env_prod(DEFAULT_ENV_PROD)
            .output(&mut factory)
            .await
            .unwrap();
    }
}
